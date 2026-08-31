use std::{io::Result, process::exit, time::Duration};

#[cfg(feature = "ppv2")]
use {
    actix_http::{HttpService, Protocol},
    actix_server::Server,
    actix_service::{IntoServiceFactory, ServiceFactoryExt, fn_service, map_config},
    actix_tls::accept::{
        TlsError,
        rustls_0_23::{Acceptor, TlsStream},
    },
    actix_web::dev::AppConfig,
};

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

#[cfg(feature = "ppv2")]
use self::proxy_protocol::ProxiedStream;

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
mod proxy_client;
#[cfg(feature = "ppv2")]
mod proxy_protocol;
mod route;
mod tls;

pub use helper::bson;
#[cfg(feature = "notebook")]
pub use {helper::utils, route::notebook::notebook_helper};

pub use route::proxy::services;

use self::{bridge_middleware::HttpRedirect, helper::maintenance_watch};

// One hour timeout for client requests
// TODO: Make this configurable
const TIMEOUT: u64 = 3600;
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
const LIFECYCLE_TIME: Duration = Duration::from_secs(3600);
#[cfg(all(feature = "notebook", feature = "lifecycle"))]
const SIGTERM_FREQ: Duration = Duration::from_secs(5);

/// Starts the Bridge server either with or without TLS. If with TLS, please ensure you have the
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

    let hclient = proxy_client::ProxyClient::new();

    // Singletons
    openid::init_once().await;
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
    let notebook_lock_handle = tokio::spawn(async move {
        let stream = LifecycleStream::new(notebook_lifecycle);
        Medium::new(LIFECYCLE_TIME, SIGTERM_FREQ, db, stream, recv.recv()).await;
    });

    let tera_data = Data::new(templating::start_template_eng());
    let mut context = Context::new();
    context.insert("application", "Bridge");
    context.insert("application_version", "v0.1.0");
    context.insert("app_name", &CONFIG.app_name);
    context.insert("company", &CONFIG.company);
    context.insert("description", &CONFIG.app_discription);
    let context = Data::new(context);

    let client_data = Data::new(client);
    let hclient_data = Data::new(hclient);
    let db = Data::new(db);
    let cache = Data::new(CACHEDB.get());
    let bridge_url: &'static str = CONFIG.bridge_url.as_str();

    let app_factory = move || {
        // clone needed due to HttpServer::new impl Fn trait and not FnOnce
        let app = App::new()
            // .wrap(bridge_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(context.clone())
            .app_data(client_data.clone())
            .app_data(hclient_data.clone())
            .app_data(db.clone())
            .app_data(cache.clone())
            .wrap(
                actix_cors::Cors::default()
                    .allowed_origin(format!("https://{}", bridge_url).as_str())
                    .allowed_origin_fn(move |origin, _req_head| {
                        origin
                            .as_bytes()
                            .ends_with(format!(".{}", bridge_url).as_bytes())
                    })
                    .max_age(3600),
            )
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(bridge_middleware::Maintainence);

        #[cfg(feature = "openwebui")]
        let app = {
            use self::bridge_middleware::{CookieCheck, OWUICookieCheck};
            app.service(
                web::scope("")
                    .guard(guard::Host(&CONFIG.owui.url))
                    .wrap(OWUICookieCheck)
                    .configure(route::openwebui::config_openwebui),
            )
            .service(
                web::scope("")
                    .guard(guard::Host(&CONFIG.moleviewer_url))
                    .wrap(CookieCheck)
                    .configure(route::moleviewer::config_moleviewer),
            )
        };

        // #[cfg(feature = "moleviewer")]
        // {
        //     todo!();
        // }

        let app = app.service(actix_files::Files::new("/static", "static"));

        #[cfg(feature = "notebook")]
        let app = app.configure(route::notebook::config_notebook);

        app.service({
            let scope = web::scope("")
                .wrap(bridge_middleware::SecurityCacheHeader)
                .wrap(bridge_middleware::custom_code_handle(
                    tera_data.clone(),
                    context.clone(),
                ))
                .configure(route::auth::config_auth)
                .configure(route::health::config_status)
                .configure(route::proxy::config_proxy)
                .configure(route::config_index)
                .configure(route::portal::config_portal)
                .configure(route::resource::config_resource)
                .configure(route::api::config_api)
                .configure(route::foo::config_foo);
            #[cfg(feature = "mcp")]
            let scope = scope.configure(route::mcp::config_mcp);
            #[cfg(feature = "openwebui")]
            let scope = scope.configure(route::openwebui::config_openwebui_manage);
            scope
        })
    };

    let ip_addr = if cfg!(debug_assertions) {
        "127.255.255.254"
    } else {
        "0.0.0.0"
    };

    if with_tls {
        // Application level https redirect, but only in release mode
        let redirect_handle = if cfg!(not(debug_assertions)) {
            #[cfg(feature = "ppv2")]
            {
                Some(tokio::spawn(
                    Server::build()
                        .bind("bridge-redirect", (ip_addr, 8000), move || {
                            let http = HttpService::build().finish(map_config(
                                App::new().wrap(HttpRedirect).into_factory(),
                                |_| AppConfig::default(),
                            ));

                            fn_service(|tcp: tokio::net::TcpStream| async move {
                                proxy_protocol::accept(tcp).await
                            })
                            .and_then(|io: ProxiedStream<tokio::net::TcpStream>| async move {
                                let peer = io.peer();
                                Ok::<_, std::io::Error>((io, Protocol::Http1, peer))
                            })
                            .and_then(http.map_err(|e| {
                                std::io::Error::other(format!("HTTP service error: {e}"))
                            }))
                        })?
                        .workers(1)
                        .run(),
                ))
            }
            #[cfg(not(feature = "ppv2"))]
            {
                Some(tokio::spawn(
                    HttpServer::new(move || App::new().wrap(HttpRedirect))
                        .workers(1)
                        .bind((ip_addr, 8000))?
                        .run(),
                ))
            }
        } else {
            None
        };

        let mut tls_config = tls::load_certs("certs/fullchain.cer", "certs/private.key");
        tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        #[cfg(feature = "ppv2")]
        {
            Server::build()
                .bind("bridge", (ip_addr, 8080), move || {
                    let acceptor = Acceptor::new(tls_config.clone());

                    let http = HttpService::build()
                        .keep_alive(actix_http::KeepAlive::Os)
                        .finish(map_config(app_factory(), |_| AppConfig::default()));

                    fn_service(|tcp: tokio::net::TcpStream| async move {
                        proxy_protocol::accept(tcp).await.map_err(TlsError::Tls)
                    })
                    .and_then(acceptor.map_err(TlsError::into_service_error))
                    .and_then(
                        |io: TlsStream<ProxiedStream<tokio::net::TcpStream>>| async move {
                            let proto = if io.get_ref().1.alpn_protocol() == Some(b"h2".as_ref()) {
                                Protocol::Http2
                            } else {
                                Protocol::Http1
                            };
                            let peer = io.get_ref().0.peer();
                            Ok::<_, TlsError<std::io::Error, actix_http::error::DispatchError>>((
                                io, proto, peer,
                            ))
                        },
                    )
                    .and_then(http.map_err(TlsError::Service))
                })?
                .run()
                .await?;
        }
        #[cfg(not(feature = "ppv2"))]
        {
            HttpServer::new(app_factory)
                .bind_rustls_0_23((ip_addr, 8080), tls_config)?
                .run()
                .await?;
        }

        if let Some(handler) = redirect_handle
            && handler.await?.is_err()
        {
            // error in shutdown for redirect server not a big deal, so just log and move on
            tracing::error!("HTTPS redirect server shutdown failed");
        };
    } else {
        HttpServer::new(app_factory)
            .bind((ip_addr, 8080))?
            .run()
            .await?;
    }

    // shutdown signal
    sender.send(()).unwrap();

    // If the lock was acquired, release it
    #[cfg(all(feature = "notebook", feature = "lifecycle"))]
    notebook_lock_handle.await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_start_server() {}
}
