use actix_web::{
    cookie::{time, Cookie},
    get,
    http::header::{self, ContentType},
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use openidconnect::{EndUserEmail, Nonce};
use serde::Deserialize;
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    auth::{
        jwt,
        openid::{self, get_openid_provider, OpenID},
    },
    config::CONFIG,
    errors::{GuardianError, Result},
    web::helper,
};

use self::deserialize::CallBackResponse;

mod deserialize;

#[get("/login")]
#[instrument]
async fn login(req: HttpRequest) -> Result<HttpResponse> {
    // get openid provider
    let provider = req.query_string();

    let openid = helper::log_errors(get_openid_provider(provider.into()))?;
    let url = openid.get_client_resources();

    // TODO: use the CsrfToken to protect against CSRF attacks

    // store nonce with the client
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
#[instrument]
async fn redirect(req: HttpRequest, data: Data<Tera>) -> Result<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let callback_response = helper::log_errors(CallBackResponse::deserialize(deserializer))?;

    let openid = helper::log_errors(get_openid_provider(openid::OpenIDProvider::W3))?;

    // get token from auth server
    code_to_response(callback_response.code, req, openid, data).await
}

#[get("/callback")]
#[instrument]
async fn callback(req: HttpRequest, data: Data<Tera>) -> Result<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let callback_response = helper::log_errors(CallBackResponse::deserialize(deserializer))?;

    let openid = helper::log_errors(get_openid_provider(openid::OpenIDProvider::IbmId))?;

    // get token from auth server
    code_to_response(callback_response.code, req, openid, data).await
}

async fn code_to_response(
    code: String,
    req: HttpRequest,
    openid: &OpenID,
    data: Data<Tera>,
) -> Result<HttpResponse> {
    let token = helper::log_errors(openid.get_token(code).await)?;

    // get nonce cookie from client
    let nonce = helper::log_errors(
        req.cookie("nonce")
            .ok_or_else(|| GuardianError::NonceCookieNotFound),
    )?;
    let nonce = Nonce::new(nonce.value().to_string());

    // verify token
    let verifier = openid.get_verifier();
    let claims = helper::log_errors(
        token
            .extra_fields()
            .id_token()
            .ok_or_else(|| GuardianError::GeneralError("No ID Token".to_string()))?
            .claims(&verifier, &nonce),
    )?;

    // get information from claims
    let email = claims
        .email()
        .unwrap_or(&EndUserEmail::new("".to_string()))
        .to_string();
    let name = helper::log_errors(|| -> Result<String> {
        let name = claims
            .given_name()
            .ok_or_else(|| GuardianError::GeneralError("No name in claims".to_string()))?;
        Ok(name
            .get(None)
            .ok_or_else(|| GuardianError::GeneralError("locale error".to_string()))?
            .to_string())
    }())?;

    // Generate guardian token
    const TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };
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
        .content_type(ContentType::html())
        .body(rendered))
}

pub fn config_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(login)
            .service(redirect)
            .service(callback),
    );
}
