use actix_web::{
    http::header::WWW_AUTHENTICATE,
    post,
    web::{self, Data},
    HttpResponse,
};
use actix_web_httpauth::extractors::basic::BasicAuth;
use mongodb::bson::doc;
use regex::Regex;
use serde_json::json;

use crate::{
    auth::jwt::validate_token,
    config::CONFIG,
    db::{
        models::{Apps, APPS},
        mongo::DB,
        Database,
    },
    errors::Result,
};

#[post("introspection")]
pub async fn introspection(
    basic: Option<BasicAuth>,
    payload: Option<web::Payload>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    let (basic, payload) = match (basic, payload) {
        (Some(basic), Some(payload)) => (basic, payload),
        _ => return Ok(invalid_response()),
    };

    let payload = payload.to_bytes().await?;
    let raw_token = String::from_utf8_lossy(&payload);

    let apps: Apps = match db.find(doc! {"client_id": basic.user_id()}, APPS).await {
        Ok(apps) => apps,
        _ => return Ok(invalid_response()),
    };

    if basic.password().is_some_and(|p| apps.client_secret.eq(p)) {
        // client is valid
        if let Some(token) = extract_token(&raw_token) {
            if validate_token(&token, &CONFIG.decoder, &CONFIG.validation).is_ok() {
                return Ok(HttpResponse::Ok().json(json!({
                    "active": true,
                })));
            }
            return Ok(HttpResponse::Ok().json(json!({
                "active": false,
            })));
        }
    };

    Ok(HttpResponse::Unauthorized()
        .append_header((WWW_AUTHENTICATE, "Basic realm=\"API Access\""))
        .json(json!({
            "error": "invalid_request",
        })))
}

#[inline]
pub fn invalid_response() -> HttpResponse {
    HttpResponse::Unauthorized()
        .append_header((WWW_AUTHENTICATE, "Basic realm=\"API Access\""))
        .json(json!({
            "error": "invalid_request",
        }))
}

#[inline]
fn extract_token(payload: &str) -> Option<String> {
    let re = Regex::new(r"(?i)token\s*=\s*([^&]+)").ok();
    re?.captures(payload)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}
