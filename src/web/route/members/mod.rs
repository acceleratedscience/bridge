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
async fn members(data: Data<Tera>) -> Result<HttpResponse> {
	let content = data.render("pages/members.html", &tera::Context::new())?;
	Ok(HttpResponse::Ok().content_type(ContentType::html()).body(content))
}

pub fn config_members(cfg: &mut web::ServiceConfig) {
	cfg.service(web::scope("/members").service(members));
	
}
