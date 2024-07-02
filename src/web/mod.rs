use std::io::Result;

use actix_web::{
    middleware::{self},
    web::Data,
    App, HttpServer,
};

use crate::templating;

mod guardian_middleware;
mod helper;
mod route;
mod tls;

pub use route::proxy::services;

pub async fn start_server(with_tls: bool) -> Result<()> {
    let server = HttpServer::new(move || {
        let tera = templating::start_template_eng();
        let tera_data = Data::new(tera);
        let client = reqwest::Client::new();
        let client_data = Data::new(client);

        App::new()
            // .wrap(guardian_middleware::HttpRedirect)
            .app_data(tera_data.clone())
            .app_data(client_data)
            .wrap(guardian_middleware::custom_code_handle(tera_data))
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(guardian_middleware::SecurityHeader)
            .configure(route::auth::config_auth)
            .configure(route::health::config_status)
            .configure(route::foo::config_foo)
            .configure(route::proxy::config_proxy)
            .configure(route::config_index)
    });

    if with_tls {
        server
            .bind_rustls_0_23(
                ("0.0.0.0", 8080),
                tls::load_certs("certs/cert.pem", "certs/key.pem"),
            )?
            .run()
            .await
    } else {
        server.bind(("0.0.0.0", 8080))?.run().await
    }
}
