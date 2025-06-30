use std::{marker::PhantomData, str::FromStr};

use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite},
    get,
    http::header::{self, ContentType},
    post,
    web::{self, Data, ReqData},
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::{Context, Tera};
use tracing::instrument;

#[cfg(feature = "observe")]
use crate::web::helper::observability_post;
use crate::{
    auth::{COOKIE_NAME, NOTEBOOK_COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME},
    config::CONFIG,
    db::{
        Database,
        models::{BridgeCookie, USER, User, UserPortalRep, UserType},
        mongo::DB,
    },
    errors::{BridgeError, Result},
    web::{
        bridge_middleware::{CookieCheck, HTMX_ERROR_RES, Htmx},
        helper::log_with_level,
        route::openwebui::OUWI_COOKIE_NAME,
        services::CATALOG,
    },
};

mod group_admin;
mod helper;
mod profile_htmx;
mod system_admin;
mod token;
mod user;
mod user_htmx;

#[instrument(skip_all, parent = None)]
#[get("")]
async fn index(data: Option<ReqData<BridgeCookie>>, db: Data<&DB>) -> Result<HttpResponse> {
    // get cookie if it exists
    match data {
        Some(r) => {
            let mut bridge_cookie = r.into_inner();

            let pipeline = db.get_user_group_pipeline(&bridge_cookie.subject);
            let mut groups = db.aggregate(pipeline, USER, PhantomData::<User>).await?;

            let mut resp = HttpResponse::SeeOther();

            if !groups.is_empty() {
                let all_resources = CATALOG.get_all_resources_by_name();

                let group_sub = groups
                    .pop()
                    .map(|u| u.group_subscriptions)
                    .and_then(|mut g| g.pop())
                    .map(|g| {
                        // only return resources that are in the catalog
                        g.into_iter()
                            .filter(|r| all_resources.contains(&r.as_str()))
                            .collect::<Vec<_>>()
                    });

                bridge_cookie.resources = group_sub;
                let bridge_cookie_json = serde_json::to_string(&bridge_cookie)?;
                let cookie = Cookie::build(COOKIE_NAME, bridge_cookie_json)
                    .same_site(SameSite::Strict)
                    .path("/")
                    .http_only(true)
                    .secure(true)
                    .finish();
                resp.cookie(cookie);
            }

            #[cfg(feature = "observe")]
            {
                observability_post("has visited the main portal", &bridge_cookie);
            }

            match bridge_cookie.user_type {
                UserType::User => Ok(resp
                    .append_header((header::LOCATION, "/portal/user"))
                    .finish()),
                UserType::GroupAdmin => Ok(resp
                    .append_header((header::LOCATION, "/portal/group_admin"))
                    .finish()),
                UserType::SystemAdmin => Ok(resp
                    .append_header((header::LOCATION, "/portal/system_admin"))
                    .finish()),
            }
        }
        None => {
            // no cookie, go back to login
            Ok(HttpResponse::SeeOther()
                .append_header((header::LOCATION, "/"))
                .finish())
        }
    }
}

#[get("search_by_email")]
async fn search_by_email(
    req: HttpRequest,
    cookie: Option<ReqData<BridgeCookie>>,
    db: Data<&DB>,
    data: Data<Tera>,
) -> Result<HttpResponse> {
    if let Some(r) = cookie {
        let bridge_cookie = r.into_inner();

        // Only Admins should be able to interact with this endpoint
        if bridge_cookie.user_type == UserType::SystemAdmin
            || bridge_cookie.user_type == UserType::GroupAdmin
        {
            // Get caller's User data from the database
            let id = ObjectId::from_str(&bridge_cookie.subject)
                .map_err(|e| BridgeError::GeneralError(e.to_string()))?;
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
            let email = urlencoding::decode(email).map_err(|e| {
                tracing::error!("{}", e);
                BridgeError::GeneralError(format!("Error parsing query string: {e}"))
            })?;

            let res = match db.search_users(&email, USER, PhantomData::<User>).await {
                Ok(documents) => documents,
                Err(e) => match e {
                    BridgeError::RecordSearchError(_) => {
                        return Ok(HttpResponse::BadRequest()
                            .append_header((HTMX_ERROR_RES, format!("No email found for {email}")))
                            .finish());
                    }
                    _ => {
                        return Err(e);
                    }
                },
            };
            let res: Vec<UserPortalRep> = res.into_iter().map(|u| u.into()).collect();

            let mut ctx = Context::new();
            ctx.insert("users", &res);

            let template = match bridge_cookie.user_type {
                UserType::SystemAdmin => "components/member_email_result_system.html",
                UserType::GroupAdmin => {
                    let group = log_with_level!(
                        user.groups.first().ok_or(BridgeError::GeneralError(
                            "Group admin is not part of any group... this should not happen"
                                .to_string()
                        )),
                        error
                    )?;

                    ctx.insert("group", group);
                    ctx.insert("group_admin", &user.email);
                    "components/member_email_result_group.html"
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

    Err(BridgeError::UserNotAllowedOnPage(
        "User is not allowed to view this page".to_string(),
    ))
}

#[post("logout")]
async fn logout(#[cfg(feature = "observe")] req: HttpRequest) -> HttpResponse {
    // clear all the cookie
    let mut cookie_remove = Cookie::build(COOKIE_NAME, "")
        .same_site(SameSite::Strict)
        .path("/")
        .http_only(true)
        .secure(true)
        .finish();
    cookie_remove.make_removal();

    let mut notebook_cookie = Cookie::build(NOTEBOOK_COOKIE_NAME, "")
        .same_site(SameSite::Strict)
        .path("/notebook")
        .http_only(true)
        .secure(true)
        .finish();
    notebook_cookie.make_removal();

    let mut openwebui_cookie = Cookie::build(OUWI_COOKIE_NAME, "")
        .domain(&CONFIG.openweb_url)
        .same_site(SameSite::Strict)
        .path("/")
        .http_only(true)
        .secure(true)
        .finish();
    openwebui_cookie.make_removal();

    let mut notebook_status_cookie = Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, "")
        .same_site(SameSite::Strict)
        .path("/")
        .http_only(true)
        .secure(true)
        .finish();
    notebook_status_cookie.make_removal();

    #[cfg(feature = "observe")]
    {
        if let Some(cookie) = req.cookie(COOKIE_NAME).map(|c| c.value().to_string()) {
            if let Ok(bridge_cookie) = serde_json::from_str::<BridgeCookie>(&cookie) {
                observability_post("has logged out from the portal", &bridge_cookie);
            }
        }
    }

    HttpResponse::Ok()
        .append_header(("HX-Redirect", "/"))
        .cookie(cookie_remove)
        .cookie(notebook_cookie)
        .cookie(notebook_status_cookie)
        .cookie(openwebui_cookie)
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
                    .service(search_by_email),
            )
            .service(index)
            .service(user::user)
            .configure(group_admin::config_group)
            .configure(system_admin::config_system),
    )
    .service(web::scope("/session").wrap(Htmx).service(logout));
}
