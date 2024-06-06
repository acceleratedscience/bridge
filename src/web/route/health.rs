use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    errors::{GuardianError, Result},
    web::helper,
};

#[get("")]
#[instrument(skip(data))]
async fn pulse(data: Data<Tera>) -> Result<HttpResponse> {
    let bod = helper::log_errors(data.render("pulse.html", &Context::new()))?;
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(bod))
}

#[get("/status")]
#[instrument]
async fn status(req: HttpRequest) -> Result<HttpResponse> {
    let header = helper::log_errors(
        req.headers()
            .get("hx-request")
            .ok_or(GuardianError::HtmxTagNotFound),
    )?;
    if header == "true" {
        Ok(HttpResponse::Ok()
            .content_type(ContentType::form_url_encoded())
            .body(r##"
                <button type="button" class="btn btn-primary" hx-get="/pulse/status" hx-target="#status" hx-swap="innerHTML">Refresh Status</button>
                <p class="text-success mt-3">OK</p>
            "##))
    } else {
        helper::log_errors(Err(GuardianError::HtmxTagNotFound))?
    }
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/pulse").service(pulse).service(status));
}
