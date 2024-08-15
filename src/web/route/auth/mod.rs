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
        openid::{self, get_openid_provider, OpenID},
        COOKIE_NAME,
    },
    db::{
        models::{GuardianCookie, User, UserType, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    log_error,
    web::helper::{self},
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

    // TODO: use the CsrfToken to protect against CSRF attacks, but since we use PCKE, we are ok

    // store nonce with the client that expires in 5 minutes, if the user does not complete the
    // authentication process in 5 minutes, they will be required to start over
    let cookie = Cookie::build("nonce", url.2.secret())
        .expires(time::OffsetDateTime::now_utc() + time::Duration::minutes(5))
        .same_site(SameSite::Lax)
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
#[instrument(skip(data, db))]
async fn redirect(req: HttpRequest, data: Data<Tera>, db: Data<&DB>) -> Result<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let callback_response = helper::log_errors(CallBackResponse::deserialize(deserializer))?;

    let openid = helper::log_errors(get_openid_provider(openid::OpenIDProvider::W3))?;

    // get token from auth server
    code_to_response(callback_response.code, req, openid, data, db).await
}

#[get("/callback")]
#[instrument(skip(data, db))]
async fn callback(req: HttpRequest, data: Data<Tera>, db: Data<&DB>) -> Result<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let callback_response = helper::log_errors(CallBackResponse::deserialize(deserializer))?;

    let openid = helper::log_errors(get_openid_provider(openid::OpenIDProvider::IbmId))?;

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
    let subject = claims.subject().to_string();
    let email = claims
        .email()
        .unwrap_or(&EndUserEmail::new("".to_string()))
        .to_string()
        .to_ascii_lowercase();
    let name = helper::log_errors(|| -> Result<String> {
        let name = claims
            .given_name()
            .ok_or_else(|| GuardianError::GeneralError("No name in claims".to_string()))?;
        Ok(name
            .get(None)
            .ok_or_else(|| GuardianError::GeneralError("locale error".to_string()))?
            .to_string())
    }())?;

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
            let r = log_error!(
                db.insert(
                    User {
                        _id: ObjectId::new(),
                        sub: subject,
                        user_name: name.clone(),
                        email: email.clone(),
                        groups: vec![],
                        user_type: UserType::User,
                        created_at: time,
                        updated_at: time,
                        last_updated_by: email,
                    },
                    USER,
                )
                .await
            )?;
            (
                r.as_object_id().ok_or_else(|| {
                    GuardianError::GeneralError("Could not convert BSON to objectid".to_string())
                })?,
                UserType::User,
            )
        }
    };

    let guardian_cookie_json = GuardianCookie {
        subject: id.to_string(),
        user_type,
    };

    let content = serde_json::to_string(&guardian_cookie_json).map_err(|e| {
        GuardianError::GeneralError(format!("Could not serialize guardian cookie: {}", e))
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
    let rendered = helper::log_errors(data.render("pages/login_success.html", &ctx))?;

    let mut cookie_remove = Cookie::build("nonce", "")
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
            .service(redirect)
            .service(callback),
    );
}
