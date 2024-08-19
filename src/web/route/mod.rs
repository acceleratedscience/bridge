use actix_web::{
    get,
    http::header,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use tera::{Context, Tera};

use crate::{auth::COOKIE_NAME, errors::Result, web::helper};

pub mod auth;
pub mod foo;
pub mod group;
pub mod groups;
pub mod health;
pub mod member;
pub mod members;
pub mod notebook;
pub mod portal;
pub mod proxy;
pub mod user;
pub mod users;

static APP_VERSION: &str = env!("CARGO_PKG_VERSION");

static APP_VERSSION: &str = env!("CARGO_PKG_VERSION");

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

pub fn config_index(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/").service(index));
}
