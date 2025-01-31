use actix_web::{
    http::header::{ContentType, WWW_AUTHENTICATE},
    post,
    web::{self, Data},
    HttpMessage, HttpRequest, HttpResponse,
};
use actix_web_httpauth::extractors::{basic::BasicAuth, bearer::BearerAuth};
use mongodb::bson::doc;
use regex::Regex;
use serde_json::json;
use tracing::error;

use crate::{
    auth::jwt::validate_token,
    config::CONFIG,
    db::{
        models::{AppPayload, Apps, User, UserType, APPS, USER},
        mongo::{ObjectID, DB},
        Database,
    },
    errors::Result,
    web::helper::generate_salt,
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

    if basic
        .password()
        .is_some_and(|p| argon2::verify_encoded(&apps.client_secret, p.as_bytes()).unwrap_or(false))
    {
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

#[post("register")]
pub async fn register_app(
    req: HttpRequest,
    token: BearerAuth,
    pl: web::Payload,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // check the content type is jpon
    if !req.content_type().eq(ContentType::json().as_ref()) {
        return Ok(HttpResponse::UnsupportedMediaType().finish());
    }
    let claim = match validate_token(token.token(), &CONFIG.decoder, &CONFIG.validation) {
        Ok(claim) => claim,
        _ => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let payload = pl.to_bytes().await?;
    let body: AppPayload = match serde_json::from_slice(&payload) {
        Ok(body) => body,
        Err(e) => {
            error!("Error deserializing payload: {}", e);
            return Ok(HttpResponse::BadRequest().finish());
        }
    };

    let user: Result<User> = db
        .find(
            doc! {"_id": ObjectID::new(claim.get_sub()).into_inner()},
            USER,
        )
        .await;
    let user = match user {
        Ok(user) => user,
        Err(e) if e.to_string().contains("Could not find any document") => {
            error!("User not found: {}", e);
            return Ok(HttpResponse::Unauthorized().finish());
        }
        _ => return Ok(HttpResponse::InternalServerError().finish()),
    };
    if !user.user_type.eq(&UserType::SystemAdmin) {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let salt = generate_salt();
    let hashed = argon2::hash_encoded(
        body.password.as_bytes(),
        salt.as_bytes(),
        &CONFIG.argon_config,
    )?;

    let app = Apps {
        client_id: body.username,
        client_secret: hashed,
        salt,
    };

    match db.insert(app, APPS).await {
        Ok(_) => (),
        Err(e) if e.to_string().contains("dup key") => {
            return Ok(HttpResponse::Conflict().finish());
        }
        _ => return Ok(HttpResponse::InternalServerError().finish()),
    }

    Ok(HttpResponse::Created().finish())
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
