use actix_web::{
    middleware::{self},
    web::Data,
    App, HttpServer,
};

use crate::templating;

mod guardian_middleware;
mod route;
mod tls;
mod helper;

pub async fn start_server(with_tls: bool) -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        let tera = templating::start_template_eng();
        let data = Data::new(tera);

        App::new()
            // .wrap(guardian_middleware::HttpRedirect)
            .app_data(data.clone())
            .wrap(guardian_middleware::custom_code_handle(data))
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Compress::default())
            .wrap(guardian_middleware::SecurityHeader)
            .configure(route::auth::config_auth)
            .configure(route::health::config_status)
            .configure(route::foo::config_foo)
            .configure(route::proxy::config_proxy)
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
