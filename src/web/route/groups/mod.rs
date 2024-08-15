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
async fn groups(data: Data<Tera>) -> Result<HttpResponse> {
    let content = data.render("pages/groups.html", &tera::Context::new())?;
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

pub fn config_groups(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/groups").service(groups));
    
}
