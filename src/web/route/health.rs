use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpMessage, HttpRequest, HttpResponse,
};
use tera::{Context, Tera};
use tracing::instrument;

#[get("")]
#[instrument]
async fn pulse(data: Data<Tera>) -> HttpResponse {
    let bod = data.render("pulse.html", &Context::new()).unwrap();
    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(bod)
}

#[get("/status")]
#[instrument]
async fn status(req: HttpRequest) -> HttpResponse {
    req.headers().get("hx-request").map_or_else(
        || HttpResponse::BadRequest().finish(),
        |val| {
            if val == "true" {
                HttpResponse::Ok()
                    .content_type(ContentType::form_url_encoded())
                    .body("<p>OK</p>")
            } else {
                HttpResponse::BadRequest().finish()
            }
        },
    )
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/pulse").service(pulse).service(status));
}
