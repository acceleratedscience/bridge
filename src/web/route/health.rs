use actix_web::{get, http::StatusCode, web, HttpResponse};
use serde_json::json;
use tracing::instrument;

#[get("")]
#[instrument]
async fn pulse() -> HttpResponse {
    let json_rep = json!({"api": "ok"});
    HttpResponse::build(StatusCode::OK).json(json_rep)
}

pub fn config_status(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/pulse").service(pulse));
}
