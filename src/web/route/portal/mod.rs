use std::{marker::PhantomData, str::FromStr};

use actix_web::{
    cookie::{Cookie, SameSite},
    get,
    http::header::{self, ContentType},
    post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::{Context, Tera};
use tracing::warn;

use crate::{
    auth::COOKIE_NAME,
    db::{
        models::{GuardianCookie, User, UserType, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::guardian_middleware::{CookieCheck, Htmx, HTMX_ERROR_RES},
    web::helper::log_with_level,
};

mod group_admin;
mod helper;
mod profile_htmx;
mod system_admin;
mod token;
mod user;
mod user_htmx;

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
    req: HttpRequest,
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
            // Get caller's User data from the database
            let id = ObjectId::from_str(&guardian_cookie.subject)
                .map_err(|e| GuardianError::GeneralError(e.to_string()))?;
            let user: User = match db.find(doc! {"_id": id}, USER).await {
                Ok(user) => user,
                Err(e) => return Err(e),
            };

            let query = req.query_string().split('=').collect::<Vec<&str>>();
            // to help the optimizer and prevent bound checks
            let query = query.as_slice();
            let length = query.len();
            // validate the query string
            let email = if length.eq(&2) && query[0] == "email" && !query[1].contains("%20") {
                query[1]
            } else {
                return Ok(HttpResponse::BadRequest()
                    .append_header((HTMX_ERROR_RES, "Invalid query string"))
                    .finish());
            };

            let res = match db.search_users(email, USER, PhantomData::<User>).await {
                Ok(documents) => documents,
                Err(e) => match e {
                    GuardianError::RecordSearchError(_) => {
                        return Ok(HttpResponse::BadRequest()
                            .append_header((
                                HTMX_ERROR_RES,
                                format!("No email found for {}", email),
                            ))
                            .finish());
                    }
                    _ => {
                        return Err(e);
                    }
                },
            };

            let mut ctx = Context::new();
            ctx.insert("users", &res);

            let template = match guardian_cookie.user_type {
                UserType::SystemAdmin => "components/user_view_result_system.html",
                UserType::GroupAdmin => {
                    let group = log_with_level!(
                        user.groups.first().ok_or(GuardianError::GeneralError(
                            "Group admin is not part of any group... this should not happen"
                                .to_string()
                        )),
                        error
                    )?;

                    dbg!(&group);
                    ctx.insert("group", group);
                    "components/user_view_result_group.html"
                }
                _ => {
                    return Ok(HttpResponse::BadRequest()
                        .append_header((HTMX_ERROR_RES, "User not allowed to view this page"))
                        .finish());
                }
            };

            let content = log_with_level!(data.render(template, &ctx), error)?;
            return Ok(HttpResponse::Ok()
                .content_type(ContentType::form_url_encoded())
                .body(content));
        }
    }

    warn!("SHIT");
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
