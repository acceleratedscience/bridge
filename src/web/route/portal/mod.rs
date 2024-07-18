use actix_web::{
    cookie::Cookie,
    get,
    http::header,
    post,
    web::{self, ReqData},
    HttpResponse,
};

use crate::{
    auth::COOKIE_NAME,
    db::models::{GuardianCookie, UserType},
    errors::Result,
    web::guardian_middleware::{CookieCheck, Htmx},
};

mod group_admin;
mod helper;
mod system_admin;
mod user;

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

#[post("logout")]
async fn logout() -> HttpResponse {
    // clear the cookie
    let mut cookie_remove = Cookie::build(COOKIE_NAME, "")
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
            .service(web::scope("/hx").wrap(Htmx).service(logout))
            .service(index)
            .service(user::user)
            .configure(group_admin::config_group)
            .configure(system_admin::config_system),
    );
}
