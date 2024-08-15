use actix_web::{
    get,
    http::{header::ContentType, StatusCode},
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use futures::StreamExt;
use reqwest::Client;
use tera::{Context, Tera};
use tracing::{error, instrument};

use crate::{
    errors::{GuardianError, Result},
    web::{guardian_middleware::Htmx, helper, services::CATALOG_URLS},
};

mod inference_services;

#[get("")]
#[instrument(skip(data))]
async fn pulse(data: Data<Tera>) -> Result<HttpResponse> {
    let bod = helper::log_errors(data.render("pages/pulse.html", &Context::new()))?;
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(bod))
}

#[get("")]
#[instrument]
async fn status(req: HttpRequest, client: Data<Client>) -> Result<HttpResponse> {
    match CATALOG_URLS.get() {
        Some(col) => {
            let is = inference_services::InferenceServicesHealth::new(col, client.as_ref().clone());

            let mut stream = is.create_stream();
            let mut builder = is.builder();

            while let Some(item) = stream.next().await {
                match item {
                    Ok((up, name, elapsed)) => {
                        if !cfg!(debug_assertions) && name == "postman" {
                            continue;
                        };
                        builder.add_inner_body(up, &name, elapsed);
                    }
                    Err(e) => {
                        // log and continue
                        error!("Error: {:?}", e);
                    }
                }
            }

            Ok(HttpResponse::Ok()
                .content_type(ContentType::form_url_encoded())
                .body(builder.render()))
        }
        None => Err(GuardianError::GeneralError(
            "CATALOG was not initialized".to_string(),
        )),
    }
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/pulse")
            .service(pulse)
            .service(web::scope("/status").wrap(Htmx).service(status)),
    );
}
