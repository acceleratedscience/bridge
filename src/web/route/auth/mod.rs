use actix_web::{
    cookie::{time, Cookie},
    get,
    http::header,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use openidconnect::{EndUserEmail, Nonce};
use serde::Deserialize;
use tera::{Context, Tera};

use crate::{
    auth::{jwt, openid},
    config::CONFIG,
    errors::{GuardianError, Result as GResult},
    web::helper,
};

use self::deserialize::CallBackResponse;

mod deserialize;

#[get("/login")]
async fn login() -> GResult<HttpResponse> {
    let openid = helper::log_errors(
        openid::OPENID
            .get()
            .ok_or_else(|| GuardianError::GeneralError("Openid not configured".to_string())),
    )?;
    let url = openid.get_client_resources();

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
async fn redirect(req: HttpRequest, data: Data<Tera>) -> GResult<HttpResponse> {
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
    let nonce = Nonce::new(nonce.value().to_string());

    let verifier = openid.get_verifier();
    let claims = token
        .extra_fields()
        .id_token()
        .unwrap()
        .claims(&verifier, &nonce)
        .unwrap();

    let email = claims
        .email()
        .unwrap_or(&EndUserEmail::new("".to_string()))
        .to_string();
    let name = if let Some(name) = claims.given_name() {
        match name.get(None) {
            Some(name) => name.to_string(),
            None => return Err(GuardianError::GeneralError("Name not found".to_string())),
        }
    } else {
        return Err(GuardianError::GeneralError("Name not found".to_string()));
    };

    const TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };
    // generate token
    let token = jwt::get_token(&CONFIG.get().unwrap().encoder, TOKEN_LIFETIME, &email)?;

    let mut ctx = Context::new();
    ctx.insert("token", &token);
    ctx.insert("name", &name);
    let rendered = helper::log_errors(data.render("token.html", &ctx))?;

    let mut cookie = Cookie::build("nonce", "")
        .http_only(true)
        .secure(true)
        .finish();
    cookie.make_removal();

    Ok(HttpResponse::Ok()
        .cookie(cookie)
        .content_type("text/html")
        .body(rendered))
}

pub fn config_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(login)
            .service(redirect),
    );
}
