use actix_web::{
    get,
    web::{Data, ReqData},
    HttpRequest, HttpResponse,
};
use tera::Tera;
use tracing::instrument;

use crate::{db::mongo::DB, errors::Result, web::{guardian_middleware::CookieSubject, helper}};

#[get("user")]
#[instrument(skip(req, db, subject))]
pub(super) async fn user(
    data: Data<Tera>,
    req: HttpRequest,
    subject: Option<ReqData<CookieSubject>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    if let Some(cookie_subject) = subject {
        let CookieSubject(subject) = cookie_subject.into_inner();
        let mut ctx = tera::Context::new();
        ctx.insert("name", &subject);
        let content = helper::log_errors(data.render("user.html", &ctx))?;

        return Ok(HttpResponse::Ok().body(content));
    }

    Ok(HttpResponse::Forbidden().finish())
}
