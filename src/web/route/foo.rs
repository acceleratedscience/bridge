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
async fn foo(data: Data<Tera>) -> Result<HttpResponse> {
    helper::log_errors(Err(GuardianError::GeneralError("Foo!".to_string())))?
}

#[get("/bar")]
async fn bar(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("portal_beta.html", &tera::Context::new())?;
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

pub fn config_foo(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/foo").service(foo).service(bar));
}
