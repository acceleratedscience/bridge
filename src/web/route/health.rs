use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpResponse,
};
use tera::{Context, Tera};
use tracing::instrument;

#[get("")]
#[instrument]
async fn pulse(data: Data<Tera>) -> HttpResponse {
    let bod = data.render("404.html", &Context::new()).unwrap();
    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(bod)
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/pulse").service(pulse));
}
