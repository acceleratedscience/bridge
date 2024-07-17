use std::{marker::PhantomData, str::FromStr};

use actix_web::{
    get,
    http::header::ContentType,
    patch,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

use crate::{
    db::{
        models::{GuardianCookie, User, UserType, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper::{self, bson},
};

const USER_PAGE: &str = "group_admin.html";

#[get("group_admin")]
#[instrument(skip(data, db, subject))]
pub(super) async fn group(
    data: Data<Tera>,
    req: HttpRequest,
    subject: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    if let Some(cookie_subject) = subject {
        let guardian_cookie = cookie_subject.into_inner();

        let id = ObjectId::from_str(&guardian_cookie.subject)
            .map_err(|e| GuardianError::GeneralError(e.to_string()))?;

        // check the db using objectid and get info on user
        let result: Result<User> = db
            .find(
                doc! {
                    "_id": id,
                },
                USER,
            )
            .await;

        let user = match result {
            Ok(user) => user,
            Err(e) => return helper::log_errors(Err(e)),
        };

        match user.user_type {
            UserType::GroupAdmin => {}
            _ => {
                return Err(GuardianError::UserNotAllowedOnPage(USER_PAGE.to_string()));
            }
        }

        let mut ctx = tera::Context::new();
        ctx.insert("name", &user.user_name);
        ctx.insert("group", &user.groups.join(", "));
        let content = helper::log_errors(data.render(USER_PAGE, &ctx))?;

        return Ok(HttpResponse::Ok().body(content));
    }

    helper::log_errors(Err(GuardianError::UserNotFound(
        "subject not passed from middleware".to_string(),
    )))
}

#[patch("user")]
async fn group_update_user(db: Data<&DB>, form: web::Form<User>) -> Result<HttpResponse> {
    let user = form.into_inner();
    let _ = db
        .update(
            doc! {"user_name": &user.user_name},
            doc! {"$set": doc! {
                "user_name": &user.user_name,
                "groups": &user.groups,
                "user_type": bson(user.user_type)?,
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
            }},
            USER,
            PhantomData::<User>,
        )
        .await?;

    let content = format!("<p>User named {} has been updated</p>", user.user_name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

pub fn config_group(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        web::scope("group_admin")
            .service(group)
            .service(group_update_user),
    );
}
