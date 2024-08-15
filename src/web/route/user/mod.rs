use actix_web::{
    get, http::header::ContentType, web::{self, Data}, HttpResponse
};
use tera::Tera;
use tracing::instrument;

use crate::{
    errors::{GuardianError, Result},
    web::helper,
};

#[get("")]
#[instrument]
async fn all_users(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/users.html", &tera::Context::new())?;
    // dbg!(&content); // Logger
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

#[get("/foo-bar")]
async fn user(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/user.html", &tera::Context::new())?;
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

pub fn config_user(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/user").service(all_users).service(user));
    
}
