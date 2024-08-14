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
async fn token(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("token.html", &tera::Context::new())?;
    // dbg!(&content); // Logger
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

pub fn config_token(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/token").service(token));
}
