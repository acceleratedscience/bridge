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
async fn all_members(data: Data<Tera>) -> Result<HttpResponse> {
    Ok(HttpResponse::Found()
        .header("Location", "/members")
        .finish())
}

#[get("/add")]
async fn member_add(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/member_add.html", &tera::Context::new())?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

pub fn config_member(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/member")
            .service(all_members)
            .service(member_add),
    );
}
