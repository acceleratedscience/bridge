use actix_web::{
    get,
    http::header::ContentType,
    web::{self, Data},
    HttpResponse,
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
    Ok(HttpResponse::Found().header("Location", "/users").finish())
}

#[get("/john-doe")]
async fn user(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/user.html", &tera::Context::new())?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

#[get("/create")]
async fn user_create(data: Data<Tera>) -> Result<HttpResponse> {
    let mut context = tera::Context::new();
    context.insert("create", &true);
    let content = data.render("pages/user_edit.html", &context)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

#[get("/john-doe/edit")]
async fn user_edit(data: Data<Tera>) -> Result<HttpResponse> {
    let mut context = tera::Context::new();
    context.insert("create", &false);
    let content = data.render("pages/user_edit.html", &context)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

pub fn config_user(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user")
            .service(all_users)
            .service(user)
            .service(user_create)
            .service(user_edit),
    );
}
