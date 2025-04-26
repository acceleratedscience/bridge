use actix_web::{
    HttpResponse, get,
    http::header::ContentType,
    web::{self, Data},
};
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    errors::{BridgeError, Result},
    web::helper,
};

#[get("")]
#[instrument]
async fn foo(data: Data<Tera>) -> Result<HttpResponse> {
    helper::log_with_level!(Err(BridgeError::GeneralError("Foo!".to_string())), error)?
}

#[get("/bar")]
async fn bar(data: Data<Tera>) -> Result<HttpResponse> {
    let mut context = Context::new();
    context.insert("main_page_title", "Open AD");
    let content = data.render("foundation.html", &context)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(content))
}

pub fn config_foo(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/foo").service(foo).service(bar));
}
