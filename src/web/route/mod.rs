use actix_web::web::Data;
use actix_web::{get, web, HttpRequest, HttpResponse};
use tera::{Context, Tera};

use crate::auth::COOKIE_NAME;
use crate::errors::Result;
use crate::web::helper;

pub mod auth;
pub mod foo;
pub mod health;
pub mod portal;
pub mod proxy;

#[get("")]
async fn index(data: Data<Tera>, req: HttpRequest) -> Result<HttpResponse> {
    // get cookie if it exists
    if let Some(_) = req.cookie(COOKIE_NAME) {
        dbg!("Welcome back!");
    }

    let ctx = Context::new();
    let rendered = helper::log_errors(data.render("login.html", &ctx))?;

    Ok(HttpResponse::Ok().body(rendered))
}

pub fn config_index(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/").service(index));
}
