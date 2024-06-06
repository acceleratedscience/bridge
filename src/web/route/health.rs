use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpRequest, HttpResponse, Result,
};
use tera::{Context, Tera};
use tracing::instrument;

use crate::errors::GuardianError;

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
async fn status(req: HttpRequest) -> Result<HttpResponse> {
    let header = req
        .headers()
        .get("hx-request")
        .ok_or(GuardianError::HtmxTagNotFound)?;
    if header == "true" {
        Ok(HttpResponse::Ok()
                    .content_type(ContentType::form_url_encoded())
                    .body(r##"
                        <button type="button" class="btn btn-primary" hx-get="/pulse/status" hx-target="#status" hx-swap="innerHTML">Refresh Status</button>
                        <p class="text-success mt-3">OK</p>
                    "##))
    } else {
        Err(GuardianError::HtmxTagNotFound)?
    }
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/pulse").service(pulse).service(status));
}
