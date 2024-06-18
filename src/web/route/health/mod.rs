use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    errors::Result,
    web::{guardian_middleware::Htmx, helper},
};

mod inference_services;

#[get("")]
#[instrument(skip(data))]
async fn pulse(data: Data<Tera>) -> Result<HttpResponse> {
    let bod = helper::log_errors(data.render("pulse.html", &Context::new()))?;
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(bod))
}

#[get("")]
#[instrument]
async fn status(req: HttpRequest) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(r##"
            <button type="button" class="btn btn-primary" hx-get="/pulse/status" hx-target="#status" hx-swap="innerHTML">Refresh Status</button>
            <p class="text-success mt-3">OK</p>
        "##))
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/pulse")
            .service(pulse)
            .service(web::scope("/status").wrap(Htmx).service(status)),
    );
}
