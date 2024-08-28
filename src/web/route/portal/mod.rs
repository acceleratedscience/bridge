use std::marker::PhantomData;

use actix_web::{
    cookie::{Cookie, SameSite},
    get,
    http::header::{self, ContentType},
    post,
    web::{self, Data, ReqData},
    HttpResponse,
};
use tera::{Context, Tera};

use crate::{
    auth::COOKIE_NAME,
    db::{
        models::{GuardianCookie, User, UserType, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::guardian_middleware::{CookieCheck, Htmx},
};

mod group_admin;
mod helper;
mod profile_htmx;
mod system_admin;
mod token;
mod user;
mod user_htmx;

pub static PROFILE_MAIN: &str = "components/token.html";

#[get("")]
async fn index(data: Option<ReqData<GuardianCookie>>) -> Result<HttpResponse> {
    // get cookie if it exists
    match data {
        Some(r) => {
            let guardian_cookie = r.into_inner();
            match guardian_cookie.user_type {
                UserType::User => Ok(HttpResponse::TemporaryRedirect()
                    .append_header((header::LOCATION, "/portal/user"))
                    .finish()),
                UserType::GroupAdmin => Ok(HttpResponse::TemporaryRedirect()
                    .append_header((header::LOCATION, "/portal/group_admin"))
                    .finish()),
                UserType::SystemAdmin => Ok(HttpResponse::TemporaryRedirect()
                    .append_header((header::LOCATION, "/portal/system_admin"))
                    .finish()),
            }
        }
        None => {
            // no cookie, go back to login
            Ok(HttpResponse::TemporaryRedirect()
                .append_header((header::LOCATION, "/"))
                .finish())
        }
    }
}

#[get("search_by_email")]
async fn search_by_email(
    cookie: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
    data: Data<Tera>,
) -> Result<HttpResponse> {
    if let Some(r) = cookie {
        let guardian_cookie = r.into_inner();

        // Only Admins should be able to interact with this endpoint
        if guardian_cookie.user_type == UserType::SystemAdmin
            || guardian_cookie.user_type == UserType::GroupAdmin
        {
            let res = db.search_users("choi", USER, PhantomData::<User>).await?;
            let mut ctx = Context::new();
            ctx.insert("users", &res);
            let content = data.render("components/user_view_result.html", &ctx)?;
            return Ok(HttpResponse::Ok()
                .content_type(ContentType::form_url_encoded())
                .body(content));
        }
    }

    Err(GuardianError::UserNotAllowedOnPage(
        "User is not allowed to view this page".to_string(),
    ))
}

#[post("logout")]
async fn logout() -> HttpResponse {
    // clear the cookie
    let mut cookie_remove = Cookie::build(COOKIE_NAME, "")
        .same_site(SameSite::Strict)
        .path("/")
        .http_only(true)
        .secure(true)
        .finish();
    cookie_remove.make_removal();

    HttpResponse::Ok()
        .append_header(("HX-Redirect", "/"))
        .cookie(cookie_remove)
        .finish()
}

pub fn config_portal(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/portal")
            .wrap(CookieCheck)
            .service(
                web::scope("/hx")
                    .wrap(Htmx)
                    .service(token::get_token_for_user)
                    .service(search_by_email)
                    .service(logout),
            )
            .service(index)
            .service(user::user)
            .configure(group_admin::config_group)
            .configure(system_admin::config_system),
    );
}
