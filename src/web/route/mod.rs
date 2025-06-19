use actix_web::{
    HttpRequest, HttpResponse, get,
    http::header,
    web::{self, Data},
};
use tera::{Context, Tera};

use crate::{
    auth::COOKIE_NAME,
    errors::Result,
    web::helper::{self},
};

pub mod auth;
pub mod foo;
pub mod health;
#[cfg(feature = "mcp")]
pub mod mcp;
#[cfg(feature = "notebook")]
pub mod notebook;
pub mod portal;
pub mod proxy;
pub mod resource;
#[cfg(feature = "openwebui")]
pub mod openwebui;

#[get("")]
async fn index(data: Data<Tera>, ctx: Data<Context>, req: HttpRequest) -> Result<HttpResponse> {
    // if cookie exists, redirect to portal
    if req.cookie(COOKIE_NAME).is_some() {
        return Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/portal"))
            .finish());
    }

    let ctx = (**ctx).clone();
    let rendered = helper::log_with_level!(data.render("pages/login.html", &ctx), error)?;

    Ok(HttpResponse::Ok().body(rendered))
}

#[get("")]
async fn maintenance(data: Data<Tera>) -> Result<HttpResponse> {
    let rendered = helper::log_with_level!(
        data.render("pages/maintenance.html", &Context::new()),
        error
    )?;
    Ok(HttpResponse::ServiceUnavailable().body(rendered))
}

// TODO: protect this endpoint with basic auth... and add db ping
#[get("")]
async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().finish())
}

pub fn config_index(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/").service(index))
        .service(web::scope("/maintenance").service(maintenance))
        .service(web::scope("/health").service(health_check));
}
