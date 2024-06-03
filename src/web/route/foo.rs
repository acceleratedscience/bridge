use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web::{self, Data};
use actix_web::{get, HttpResponse};
use tera::{Context, Tera};
use tracing::instrument;

#[get("")]
#[instrument]
async fn foo(data: Data<Tera>) -> HttpResponse {
    let bod = data.render("500.html", &Context::new()).unwrap();
    HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
        .content_type(ContentType::html())
        .body(bod)
}

pub fn config_foo(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/foo").service(foo));
}
