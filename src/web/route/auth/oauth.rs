use std::{marker::PhantomData, str::FromStr};

use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, get,
    http::header::{ContentType, WWW_AUTHENTICATE},
    post,
    web::{self, Data, ReqData},
};
use actix_web_httpauth::extractors::{basic::BasicAuth, bearer::BearerAuth};
use mongodb::bson::{doc, oid::ObjectId};
use regex::Regex;
use serde_json::{Value, json};
use tracing::error;

use crate::{
    auth::jwt::{self, validate_token},
    config::{AUD, CONFIG},
    db::{
        Database,
        models::{
            APPS, AppPayload, Apps, BridgeCookie, GROUP, Group, GroupSubs, USER, User, UserType,
        },
        mongo::{DB, ObjectID},
    },
    errors::{BridgeError, Result},
    web::{
        helper::{self, generate_salt},
        route::auth::{COOKIE_TOKEN_LIFETIME, TOKEN_LIFETIME},
    },
};

#[post("token")]
pub async fn get_token(
    subject: Option<ReqData<BridgeCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    let bc = match subject {
        Some(cookie_subject) => cookie_subject.into_inner(),
        None => {
            return helper::log_with_level!(
                Err(BridgeError::UserNotFound(
                    "subject not passed from middleware".to_string(),
                )),
                error
            );
        }
    };

    let id =
        ObjectId::from_str(&bc.subject).map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let (token, _, _) = generate_token_with_cookie(&id, &bc, &db, COOKIE_TOKEN_LIFETIME).await?;

    let payload = json!({
        "access_token": token,
        "token_type": "Bearer",
        "expires_in": TOKEN_LIFETIME,
    });

    Ok(HttpResponse::Ok().json(payload))
}

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
            if let Ok(claims) = validate_token(&token, &CONFIG.decoder, &CONFIG.validation) {
                let pipeline = db.get_user_group_pipeline(claims.get_sub());
                let docs = helper::log_with_level!(
                    db.aggregate(pipeline, USER, PhantomData::<User>).await,
                    error
                )?;
                // TODO: we currently only support one group per user... so take the first element
                // but in the future we will have handle this differently
                let mut json = json!({
                    "active": true,
                    "sub": claims.get_sub(),
                    "client_id": apps.client_id,
                });

                // Only insert the group_id if the user is in a group
                if let Some(GroupSubs { group_id, .. }) = docs.first()
                    && let Some(group_id) = group_id.first()
                    && let Value::Object(map) = &mut json
                {
                    map.insert("group_id".to_string(), Value::String(group_id.to_string()));
                };

                return Ok(HttpResponse::Ok().json(json));
            }
            return Ok(HttpResponse::Ok().json(json!({
                "active": false,
                "client_id": apps.client_id,
            })));
        }
    };

    Ok(HttpResponse::Unauthorized()
        .append_header((WWW_AUTHENTICATE, "Basic realm=\"API Access\""))
        .json(json!({
            "error": "invalid_request",
        })))
}

#[get("/.well-known/jwks.json")]
pub async fn jwks() -> Result<HttpResponse> {
    let payload = json!({
        "keys": vec![&CONFIG.jwk],
    });
    Ok(HttpResponse::Ok().json(payload))
}

#[post("register")]
pub async fn register_app(
    req: HttpRequest,
    token: BearerAuth,
    pl: web::Payload,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // check the content type is json
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

pub async fn generate_token_with_cookie(
    id: &ObjectId,
    bc: &BridgeCookie,
    db: &DB,
    token_lifetime: usize,
) -> Result<(String, String, User)> {
    // get information about user
    let user: User = helper::log_with_level!(
        db.find(
            doc! {
                "_id": id,
            },
            USER,
        )
        .await,
        error
    )?;

    let scp = if user.groups.is_empty() {
        vec!["".to_string()]
    } else {
        // get models
        let group: Group = helper::log_with_level!(
            db.find(
                doc! {
                    "name": &user.groups[0]
                },
                GROUP,
            )
            .await,
            error
        )?;
        group.subscriptions
    };

    // Generate bridge token
    let (token, exp) = helper::log_with_level!(
        jwt::get_token_and_exp(&CONFIG.encoder, token_lifetime, &bc.subject, AUD[0], scp),
        error
    )?;
    Ok((token, exp, user))
}
