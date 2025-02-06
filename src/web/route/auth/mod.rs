use actix_web::{
    cookie::{time, Cookie, SameSite},
    get,
    http::header::{self, ContentType},
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use openidconnect::{EndUserEmail, Nonce};
use serde::Deserialize;
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    auth::{
        openid::{get_openid_provider, OpenID, OpenIDProvider},
        COOKIE_NAME,
    },
    db::{
        models::{BridgeCookie, User, UserType, USER},
        mongo::DB,
        Database,
    },
    errors::{BridgeError, Result},
    web::helper::{self},
};

use self::{
    deserialize::CallBackResponse,
    oauth::{introspection, jwks, register_app},
};

mod deserialize;
mod oauth;

const NONCE_COOKIE: &str = "nonce";

#[get("/login")]
#[instrument]
async fn login(req: HttpRequest) -> Result<HttpResponse> {
    // get openid provider
    let provider = req.query_string();

    let openid = helper::log_with_level!(get_openid_provider(provider.into()), error)?;
    let url = openid.get_client_resources();

    // TODO: use the CsrfToken to protect against CSRF attacks, but since we use PCKE, we are ok

    // store nonce with the client that expires in 5 minutes, if the user does not complete the
    // authentication process in 5 minutes, they will be required to start over
    let cookie = Cookie::build(NONCE_COOKIE, url.2.secret())
        .expires(time::OffsetDateTime::now_utc() + time::Duration::minutes(5))
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(true)
        .finish();

    // redirect to auth server
    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, url.0.to_string()))
        .cookie(cookie)
        .finish())
}

#[get("/callback/{provider}")]
#[instrument(skip(data, db))]
async fn callback(
    req: HttpRequest,
    data: Data<Tera>,
    db: Data<&DB>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let callback_response =
        helper::log_with_level!(CallBackResponse::deserialize(deserializer), error)?;

    let openid_kind = Into::<OpenIDProvider>::into(path.into_inner().as_str());
    if let OpenIDProvider::None = openid_kind {
        return helper::log_with_level!(
            Err(BridgeError::GeneralError(
                "Invalid Open id connect provider".to_string()
            )),
            error
        );
    }

    let openid = helper::log_with_level!(get_openid_provider(openid_kind), error)?;

    // get token from auth server
    code_to_response(callback_response.code, req, openid, data, db).await
}

async fn code_to_response(
    code: String,
    req: HttpRequest,
    openid: &OpenID,
    data: Data<Tera>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    let token = helper::log_with_level!(openid.get_token(code).await, error)?;

    // get nonce cookie from client
    let nonce = helper::log_with_level!(
        req.cookie(NONCE_COOKIE)
            .ok_or_else(|| BridgeError::NonceCookieNotFound),
        error
    )?;
    let nonce = Nonce::new(nonce.value().to_string());

    // verify token
    let verifier = openid.get_verifier();
    let claims = helper::log_with_level!(
        token
            .extra_fields()
            .id_token()
            .ok_or_else(|| BridgeError::GeneralError("No ID Token".to_string()))?
            .claims(&verifier, &nonce),
        error
    )?;

    // get information from claims
    let subject = claims.subject().to_string();
    let email = claims
        .email()
        .unwrap_or(&EndUserEmail::new("".to_string()))
        .to_string()
        .to_ascii_lowercase();
    let name = helper::log_with_level!(
        || -> Result<String> {
            let name = claims
                .given_name()
                .ok_or_else(|| BridgeError::GeneralError("No name in claims".to_string()))?;
            Ok(name
                .get(None)
                .ok_or_else(|| BridgeError::GeneralError("locale error".to_string()))?
                .to_string())
        }(),
        error
    )?;

    // look up user in database
    let r: Result<User> = db
        .find(
            doc! {
                "sub": &subject
            },
            USER,
        )
        .await;

    let (id, user_type) = match r {
        Ok(user) => (user._id, user.user_type),
        // user not found, create user
        Err(_) => {
            // get current time in time after unix epoch
            let time = time::OffsetDateTime::now_utc();
            // add user to the DB as a new user
            let r = helper::log_with_level!(
                db.insert(
                    User {
                        _id: ObjectId::new(),
                        sub: subject,
                        user_name: name.clone(),
                        email: email.clone(),
                        groups: vec![],
                        user_type: UserType::User,
                        token: None,
                        notebook: None,
                        created_at: time,
                        updated_at: time,
                        last_updated_by: email,
                    },
                    USER,
                )
                .await,
                error
            )?;
            (
                r.as_object_id().ok_or_else(|| {
                    BridgeError::GeneralError("Could not convert BSON to objectid".to_string())
                })?,
                UserType::User,
            )
        }
    };

    let bridge_cookie_json = BridgeCookie {
        subject: id.to_string(),
        user_type,
        config: None,
    };

    let content = serde_json::to_string(&bridge_cookie_json).map_err(|e| {
        BridgeError::GeneralError(format!("Could not serialize bridge cookie: {}", e))
    })?;

    // create cookie for all routes for this user
    // middleware will check for this cookie and and safeguard specific routes
    // TODO: look into doing session management that stores a dynamic key into the cookie
    let cookie = Cookie::build(COOKIE_NAME, content)
        .same_site(SameSite::Strict)
        .expires(time::OffsetDateTime::now_utc() + time::Duration::days(1))
        .path("/")
        .http_only(true)
        .secure(true)
        .finish();

    let mut ctx = Context::new();
    ctx.insert("name", &name);
    let rendered = helper::log_with_level!(data.render("pages/login_success.html", &ctx), error)?;

    let mut cookie_remove = Cookie::build(NONCE_COOKIE, "")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(true)
        .finish();
    cookie_remove.make_removal();

    Ok(HttpResponse::Ok()
        .cookie(cookie_remove)
        .cookie(cookie)
        .content_type(ContentType::html())
        .body(rendered))
}

pub fn config_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(login)
            .service(callback)
            .service(introspection)
            .service(register_app)
            .service(jwks),
    );
}
