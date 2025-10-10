use std::{io::Result, process::exit, time::Duration};

#[cfg(feature = "openwebui")]
use actix_web::guard;
use actix_web::{
    App, HttpServer,
    middleware::{self},
    web::{self, Data},
};
use tera::Context;
use tokio::sync::broadcast::channel;
use tracing::level_filters::LevelFilter;

#[cfg(feature = "notebook")]
use crate::kube::{self};
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use crate::kube::{LifecycleStream, Medium, notebook_lifecycle};

use crate::{
    auth::openid,
    config::CONFIG,
    db::{
        keydb::{CACHEDB, CacheDB},
        mongo::{DB, DBCONN, DBNAME},
    },
    logger, templating,
};

mod bridge_middleware;
mod helper;
mod route;
mod tls;

pub use helper::bson;
#[cfg(feature = "notebook")]
pub use helper::utils;

#[cfg(feature = "notebook")]
pub use route::notebook::notebook_helper;
pub use route::proxy::services;

use self::{bridge_middleware::HttpRedirect, helper::maintenance_watch};

// One hour timeout for client requests
// TODO: Make this configurable
const TIMEOUT: u64 = 3600;
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
const LIFECYCLE_TIME: Duration = Duration::from_secs(3600);
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
const SIGTERM_FREQ: Duration = Duration::from_secs(5);

/// Starts the OpenBridge server either with or without TLS. If with TLS, please ensure you have the
/// appropriate certs in the `certs` directory.
///
/// # Example
/// ```ignore
/// use bridge::web::start_server;
/// let tls = true;
/// let result = start_server(tls).await;
///
/// match result {
///    Ok(_) => println!("Server ran..."),
///    Err(e) => eprintln!("Error starting server: {e}"),
/// }
/// ```
pub async fn start_server(with_tls: bool) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT))
        .build()
        .expect("Cannot create reqwest client");

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Cannot install default provider with ring");

    // Singletons
    openid::init_once().await; // <-- TEMP DISABLED TO BYPASS LOGIN
    if let Err(e) = DB::init_once(&DBNAME).await {
        eprintln!("{e}");
        exit(1);
    }
    if let Err(e) = CacheDB::init_once().await {
        // we don't want to make caching a hard requirement for now
        eprintln!("{e}: continuing without caching");
    }
    let db = match DBCONN.get() {
        Some(db) => db,
        None => {
            eprintln!("DB Connection not found... Is the DB running?");
            exit(1);
        }
    };
    #[cfg(feature = "notebook")]
    kube::init_once().await;

    #[allow(unused_mut)]
    #[allow(unused_variables)]
    let (sender, mut recv) = channel::<()>(1);
    let tx = sender.clone();

    // Logger and this is not configurable by the caller
    if cfg!(debug_assertions) {
        logger::start_logger(LevelFilter::DEBUG, client.clone(), tx);
    } else {
        logger::start_logger(LevelFilter::INFO, client.clone(), tx);
    }

    // Launch maintainence window watcher if cache is available
    let _ = maintenance_watch();

    // Lifecycle with "advisory lock"
    #[cfg(all(feature = "notebook", feature = "lifecycle"))]
    let handle = tokio::spawn(async move {
        let stream = LifecycleStream::new(notebook_lifecycle);
        Medium::new(LIFECYCLE_TIME, SIGTERM_FREQ, db, stream, recv.recv()).await;
    });

    let server = HttpServer::new(move || {
        let tera_data = Data::new(templating::start_template_eng());
        let mut context = Context::new();
        context.insert("application", "OpenBridge");
        context.insert("application_version", "v0.1.0");
        context.insert("app_name", &CONFIG.app_name);
        context.insert("company", &CONFIG.company);
        context.insert("description", &CONFIG.app_discription);
        let context = Data::new(context);

        // clone needed due to HttpServer::new impl Fn trait and not FnOnce
        let client_data = Data::new(client.clone());
        let db = Data::new(db);
        let cache = Data::new(CACHEDB.get());

        let app = App::new()
            // .wrap(bridge_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(context.clone())
            .app_data(client_data)
            .app_data(db)
            .app_data(cache)
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(bridge_middleware::Maintainence);

        #[cfg(feature = "openwebui")]
        let app = {
            use self::bridge_middleware::OWUICookieCheck;
            app.service(
                web::scope("")
                    .guard(guard::Host(&CONFIG.openweb_url))
                    .wrap(OWUICookieCheck)
                    .configure(route::openwebui::config_openwebui),
            )
            .service(
                web::scope("")
                    .guard(guard::Host(&CONFIG.moleviewer_url))
                    .wrap(OWUICookieCheck)
                    .configure(route::openwebui::config_moleviewer),
            )
        };

        let app = app.service(actix_files::Files::new("/static", "static"));

        #[cfg(feature = "notebook")]
        let app = app.configure(route::notebook::config_notebook);

        app.service({
            let scope = web::scope("")
                .wrap(bridge_middleware::SecurityCacheHeader)
                .wrap(bridge_middleware::custom_code_handle(tera_data, context))
                .configure(route::auth::config_auth)
                .configure(route::health::config_status)
                .configure(route::proxy::config_proxy)
                .configure(route::config_index)
                .configure(route::portal::config_portal)
                .configure(route::resource::config_resource)
                .configure(route::foo::config_foo);
            #[cfg(feature = "mcp")]
            let scope = scope.configure(route::mcp::config_mcp);
            scope
        })
    });

    if with_tls {
        // Application level https redirect, but only in release mode
        let redirect_handle = if cfg!(not(debug_assertions)) {
            Some(tokio::spawn(
                HttpServer::new(move || App::new().wrap(HttpRedirect))
                    .workers(1)
                    .bind(("0.0.0.0", 8000))?
                    .run(),
            ))
        } else {
            None
        };

        server
            .bind_rustls_0_23(
                ("0.0.0.0", 8080),
                tls::load_certs("certs/fullchain.cer", "certs/open.accelerate.science.key"),
            )?
            .run()
            .await?;

        if let Some(handler) = redirect_handle {
            handler.await??;
        }
    } else {
        server.bind(("0.0.0.0", 8080))?.run().await?;
    }

    // shutdown signal
    sender.send(()).unwrap();

    // If the lock was acquired, release it
    #[cfg(all(feature = "notebook", feature = "lifecycle"))]
    handle.await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_start_server() {}
}
