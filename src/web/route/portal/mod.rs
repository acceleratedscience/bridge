use actix_web::http::header;
use actix_web::web::ReqData;
use actix_web::{get, web, HttpResponse};

use crate::db::models::{GuardianCookie, UserType};
use crate::errors::Result;
use crate::web::guardian_middleware::CookieCheck;

mod group_admin;
mod system_admin;
mod user;
mod helper;

#[get("")]
async fn index(data: Option<ReqData<GuardianCookie>>) -> Result<HttpResponse> {
    // get cookie if it exists
    match data {
        Some(r) => {
            let guardian_cookie = r.into_inner();
            match guardian_cookie.user_type {
                UserType::User => Ok(HttpResponse::PermanentRedirect()
                    .append_header((header::LOCATION, "/portal/user"))
                    .finish()),
                UserType::GroupAdmin => Ok(HttpResponse::PermanentRedirect()
                    .append_header((header::LOCATION, "/portal/group_admin"))
                    .finish()),
                UserType::SystemAdmin => Ok(HttpResponse::PermanentRedirect()
                    .append_header((header::LOCATION, "/portal/system_admin"))
                    .finish()),
            }
        }
        None => {
            // no cookie, go back to login
            Ok(HttpResponse::PermanentRedirect()
                .append_header((header::LOCATION, "/"))
                .finish())
        }
    }
}

pub fn config_portal(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/portal")
            .wrap(CookieCheck)
            .service(index)
            .service(user::user)
            .configure(group_admin::config_group)
            .configure(system_admin::config_system),
    );
}
