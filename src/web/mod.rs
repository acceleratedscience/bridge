use actix_web::{
    middleware::{self, Logger},
    App, HttpServer,
};

mod guardian_middleware;
mod route;
mod tls;

pub async fn start_server(with_tls: bool) -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        App::new()
            // .wrap(guardian_middleware::HttpRedirect)
            .wrap(guardian_middleware::custom_404_handle())
            .wrap(middleware::NormalizePath::trim())
            .wrap(Logger::default())
            .wrap(middleware::Compress::default())
            .configure(route::health::config_status)
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
