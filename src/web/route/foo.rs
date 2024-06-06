use actix_web::{get, web::{self, Data}, HttpResponse, Result};
use tera::Tera;
use tracing::instrument;

use crate::errors::GuardianError;

#[get("")]
#[instrument]
async fn foo(data: Data<Tera>) -> Result<HttpResponse> {
    Err(GuardianError::GeneralError("Foo!".to_string()))?
}

pub fn config_foo(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/foo").service(foo));
}
