use std::{io::Result, process::exit, time::Duration};

#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use std::pin::pin;

use actix_web::{
    middleware::{self},
    web::{self, Data},
    App, HttpServer,
};
use tracing::{error, level_filters::LevelFilter, warn};

#[cfg(feature = "notebook")]
use crate::kube::{self};
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use crate::kube::{notebook_lifecycle, LifecycleStream, Medium};
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use futures::future::select;

use crate::{
    auth::openid,
    db::{
        keydb::{CacheDB, CACHEDB},
        mongo::{DB, DBCONN, DBNAME},
    },
    logger::Logger,
    templating,
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

use self::helper::maintenance_watch;

// One hour timeout for client requests
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
    // Not configurable by the caller
    // Either INFO or WARN based on release mode
    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::DEBUG);
    } else {
        Logger::start(LevelFilter::INFO);
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Cannot install default provider");

    // Singletons
    openid::init_once().await;
    if let Err(e) = DB::init_once(DBNAME).await {
        error!("{e}");
        exit(1);
    }
    if let Err(e) = CacheDB::init_once().await {
        // we don't want to make caching a hard requirement for now
        warn!("{e}: continuing without caching");
    }
    let db = match DBCONN.get() {
        Some(db) => db,
        None => {
            error!("DB Connection not found... Is the DB running?");
            exit(1);
        }
    };
    #[cfg(feature = "notebook")]
    kube::init_once().await;

    let _ = maintenance_watch();

    // Lifecycle with "advisory lock"
    #[cfg(all(feature = "notebook", feature = "lifecycle"))]
    let handle = tokio::spawn(async move {
        let stream = LifecycleStream::new(notebook_lifecycle);
        Medium::new(
            LIFECYCLE_TIME,
            SIGTERM_FREQ,
            db,
            stream,
            select(
                pin!(
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("Cannot establish SIGTERM signal")
                        .recv()
                ),
                pin!(
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                        .expect("Cannot establish SIGINT signal")
                        .recv()
                ),
            ),
        )
        .await;
    });

    let server = HttpServer::new(move || {
        let tera_data = Data::new(templating::start_template_eng());
        let client_data = Data::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(TIMEOUT))
                .build()
                .expect("Cannot create reqwest client"),
        );
        let db = Data::new(db);
        let cache = Data::new(CACHEDB.get());

        let app = App::new()
            // .wrap(bridge_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(client_data)
            .app_data(db)
            .app_data(cache)
            .wrap(bridge_middleware::custom_code_handle(tera_data))
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(bridge_middleware::Maintainence)
            .service(actix_files::Files::new("/static", "static"));
        #[cfg(feature = "notebook")]
        let app = app.configure(route::notebook::config_notebook);
        app.service(
            web::scope("")
                .wrap(bridge_middleware::SecurityHeader)
                .configure(route::auth::config_auth)
                .configure(route::health::config_status)
                .configure(route::proxy::config_proxy)
                .configure(route::config_index)
                .configure(route::portal::config_portal)
                .configure(route::resource::config_resource)
                .configure(route::foo::config_foo),
        )
    });

    if with_tls {
        server
            .bind_rustls_0_23(
                ("0.0.0.0", 8080),
                tls::load_certs("certs/fullchain.cer", "certs/open.accelerator.cafe.key"),
            )?
            .run()
            .await?;
    } else {
        server.bind(("0.0.0.0", 8080))?.run().await?;
    }

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
