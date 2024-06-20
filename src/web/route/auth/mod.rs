use actix_web::{
    cookie::{time, Cookie},
    get,
    http::header,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use openidconnect::Nonce;
use serde::Deserialize;
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    auth::{jwt, openid},
    config::CONFIG,
    errors::{GuardianError, Result as GResult},
    web::helper,
};

use self::deserialize::{CallBackResponse, TokenRequest};

mod deserialize;

#[get("/get_token")]
#[instrument(skip(data))]
async fn get_token(data: Data<Tera>, req: HttpRequest) -> GResult<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let q = match TokenRequest::deserialize(deserializer) {
        Ok(q) => q,
        Err(e) => {
            return helper::log_errors(Err(GuardianError::QueryDeserializeError(e.to_string())))
        }
    };

    if q.admin != "thisisbadsecurity" {
        return helper::log_errors(Err(GuardianError::NotAdmin));
    }

    const TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };

    // generate token
    let token = jwt::get_token(&CONFIG.get().unwrap().encoder, TOKEN_LIFETIME, &q.username)?;

    if let Some(true) = q.gui {
        let mut ctx = Context::new();
        ctx.insert("token", &token);
        ctx.insert("name", &q.username);
        let rendered = helper::log_errors(data.render("token.html", &ctx))?;

        Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
    } else {
        Ok(HttpResponse::Ok().json(token))
    }
}

#[get("/login")]
async fn login() -> GResult<HttpResponse> {
    let openid = helper::log_errors(
        openid::OPENID
            .get()
            .ok_or_else(|| GuardianError::GeneralError("Openid not configured".to_string())),
    )?;
    let url = openid.get_client_resources();
    dbg!(&url.2.secret());
    // store nonce as a cookie on client
    let cookie = Cookie::build("nonce", url.2.secret())
        .expires(time::OffsetDateTime::now_utc() + time::Duration::minutes(5))
        .http_only(true)
        .secure(true)
        .finish();

    // redirect to auth server
    Ok(HttpResponse::TemporaryRedirect()
        .append_header((header::LOCATION, url.0.to_string()))
        .cookie(cookie)
        .finish())
}

#[get("/redirect")]
async fn redirect(req: HttpRequest) -> GResult<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let q = match CallBackResponse::deserialize(deserializer) {
        Ok(q) => q,
        Err(e) => {
            return helper::log_errors(Err(GuardianError::QueryDeserializeError(e.to_string())))
        }
    };
    let openid = helper::log_errors(
        openid::OPENID
            .get()
            .ok_or_else(|| GuardianError::GeneralError("Openid not configured".to_string())),
    )?;
    let token = openid.get_token(q.code).await?;

    // get nonce cookie from client
    let nonce = req
        .cookie("nonce")
        .ok_or_else(|| GuardianError::GeneralError("Nonce cookie not found".to_string()))?;
    let nonce = Nonce::new(nonce.to_string());

    dbg!(&nonce.secret());

    let mut cookie = Cookie::build("nonce", "")
        .http_only(true)
        .secure(true)
        .finish();
    cookie.make_removal();

    Ok(HttpResponse::Ok().cookie(cookie).finish())
}

pub fn config_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(get_token)
            .service(login)
            .service(redirect),
    );
}
