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
async fn all_groups(data: Data<Tera>) -> Result<HttpResponse> {
    Ok(HttpResponse::Found().header("Location", "/groups").finish())
}

#[get("/foo-bar")]
async fn group(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/group.html", &tera::Context::new())?;
    // dbg!(&content); // Logger
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

#[get("/create")]
async fn group_create(data: Data<Tera>) -> Result<HttpResponse> {
    let mut context = tera::Context::new();
    context.insert("create", &true);
    let content = data.render("pages/group_edit.html", &context)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

#[get("/foo-bar/edit")]
async fn group_edit(data: Data<Tera>) -> Result<HttpResponse> {
    let mut context = tera::Context::new();
    context.insert("create", &false);
    let content = data.render("pages/group_edit.html", &context)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

pub fn config_group(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/group")
            .service(all_groups)
            .service(group)
            .service(group_create)
            .service(group_edit),
    );
}
