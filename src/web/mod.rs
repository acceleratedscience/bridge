use std::{io::Result, process::exit};

use actix_web::{
    middleware::{self},
    web::{self, Data},
    App, HttpServer,
};
use tracing::error;

use crate::{db::mongo::DBCONN, templating};

mod guardian_middleware;
mod helper;
mod route;
mod tls;

pub use route::proxy::services;

pub async fn start_server(with_tls: bool) -> Result<()> {
    let server = HttpServer::new(move || {
        let tera = templating::start_template_eng();
        let tera_data = Data::new(tera);
        let client_data = Data::new(reqwest::Client::new());
        let db = Data::new({
            match DBCONN.get() {
                Some(db) => db,
                None => {
                    error!("DB Connection not found... this should not have happened");
                    exit(1);
                }
            }
        });

        App::new()
            // .wrap(guardian_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(client_data)
            .app_data(db)
            .wrap(guardian_middleware::custom_code_handle(tera_data))
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            // .configure(route::notebook::config_notebook)
            .service(actix_files::Files::new("/static", "static"))
            .service(
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
            .await
    } else {
        server.bind(("0.0.0.0", 8080))?.run().await
    }
}
