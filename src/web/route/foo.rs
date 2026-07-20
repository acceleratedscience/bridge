use actix_web::{
    HttpResponse, get,
    http::header::ContentType,
    web::{self, Data},
};
use tera::{Context, Tera};

use crate::errors::Result;

static FRONTEND_PATH: &str = if cfg!(debug_assertions) {
    "target/dx/frontend/debug/web/public"
} else {
    "frontend/public"
};

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
    cfg.service(
        web::scope("/foo")
            .service(bar)
            .service(actix_files::Files::new("", FRONTEND_PATH).index_file("index.html")),
    );
}
