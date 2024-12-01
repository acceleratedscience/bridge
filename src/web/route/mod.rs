use actix_web::{
    get,
    http::header,
    post,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use tera::{Context, Tera};

use crate::{
    auth::COOKIE_NAME,
    db::models::MaintenanceMode,
    errors::Result,
    web::helper::{self, payload_to_struct},
};

pub mod auth;
pub mod foo;
pub mod health;
pub mod portal;
pub mod proxy;
#[cfg(feature = "notebook")]
pub mod notebook;

static APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[get("")]
async fn index(data: Data<Tera>, req: HttpRequest) -> Result<HttpResponse> {
    // if cookie exists, redirect to portal
    if req.cookie(COOKIE_NAME).is_some() {
        return Ok(HttpResponse::TemporaryRedirect()
            .append_header((header::LOCATION, "/portal"))
            .finish());
    }

    let mut ctx = Context::new();
    ctx.insert("version", APP_VERSION);
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

#[post("mode")]
async fn mode(_req: HttpRequest, pl: web::Payload) -> Result<HttpResponse> {
    let _payload = payload_to_struct::<MaintenanceMode>(pl).await?;
    Ok(HttpResponse::Ok().finish())
}

pub fn config_index(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/").service(index))
        .service(web::scope("/maintenance").service(maintenance));
}
