use std::{io::Result, process::exit, time::Duration};

#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use std::pin::pin;

use actix_web::{
    middleware::{self},
    web::{self, Data},
    App, HttpServer,
};
use tracing::{error, level_filters::LevelFilter};

#[cfg(feature = "notebook")]
use crate::kube::{self};
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use crate::kube::{notebook_lifecycle, LifecycleStream, Medium};
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
use futures::future::select;

use crate::{
    auth::openid,
    db::mongo::{DB, DBCONN},
    logger::Logger,
    templating,
};

mod guardian_middleware;
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
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
const AD_LOCK: &str = "guardian-lock";

/// Starts the Guardian server either with or without TLS. If with TLS, please ensure you have the
/// appropriate certs in the `certs` directory.
///
/// # Example
/// ```ignore
/// use guardian::web::start_server;
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
        Logger::start(LevelFilter::INFO);
    } else {
        Logger::start(LevelFilter::WARN);
    }

    #[cfg(feature = "notebook")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Cannot install default provider");

    // Singletons
    openid::init_once().await;
    if let Err(e) = DB::init_once("guardian").await {
        error!("{e}");
        exit(1);
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
    let handle = if (db.get_lock(AD_LOCK).await).is_ok() {
        tracing::info!("Look at me, I'm the captain now...");
        Some(tokio::spawn(async move {
            let stream = LifecycleStream::new(notebook_lifecycle);
            Medium::new(
                LIFECYCLE_TIME,
                SIGTERM_FREQ,
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
        }))
    } else {
        None
    };

    let server = HttpServer::new(move || {
        let tera = templating::start_template_eng();
        let tera_data = Data::new(tera);
        let client_data = Data::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(TIMEOUT))
                .build()
                .expect("Cannot create reqwest client"),
        );
        let db = Data::new(db);

        let app = App::new()
            // .wrap(guardian_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(client_data)
            .app_data(db)
            .wrap(guardian_middleware::custom_code_handle(tera_data))
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(guardian_middleware::Maintainence)
            .service(actix_files::Files::new("/static", "static"));
        #[cfg(feature = "notebook")]
        let app = app.configure(route::notebook::config_notebook);
        app.service(
            web::scope("")
                .wrap(guardian_middleware::SecurityHeader)
                .configure(route::auth::config_auth)
                .configure(route::health::config_status)
                .configure(route::proxy::config_proxy)
                .configure(route::config_index)
                .configure(route::portal::config_portal)
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
    if let Some(handle) = handle {
        handle.await?;
        db.release_lock(AD_LOCK)
            .await
            .expect("Cannot release advisory lock");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_start_server() {}
}
