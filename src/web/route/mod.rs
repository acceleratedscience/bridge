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
pub mod health;
pub mod portal;
pub mod proxy;

#[get("")]
async fn index(data: Data<Tera>, req: HttpRequest) -> Result<HttpResponse> {
    // if cookie exists, redirect to portal
    if let Some(c) = req.cookie(COOKIE_NAME) {
        return Ok(HttpResponse::TemporaryRedirect()
            .append_header((header::LOCATION, "/portal"))
            .finish());
    }

    let ctx = Context::new();
    let rendered = helper::log_errors(data.render("login.html", &ctx))?;

    Ok(HttpResponse::Ok().body(rendered))
}

pub fn config_index(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/").service(index));
}
