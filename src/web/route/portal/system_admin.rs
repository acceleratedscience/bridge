use std::{marker::PhantomData, str::FromStr};

use actix_web::web;
use actix_web::{
    get,
    http::header::ContentType,
    patch, post,
    web::{Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

use crate::web::helper::bson;
use crate::{
    db::{
        models::{Group, GuardianCookie, User, UserType, GROUP, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper::{self},
};

const USER_PAGE: &str = "system_admin.html";

#[get("")]
#[instrument(skip(data, db, subject))]
pub(super) async fn system(
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
            UserType::SystemAdmin => {}
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

#[post("group")]
async fn system_create_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
    let mut group = form.into_inner();
    let now = time::OffsetDateTime::now_utc();
    group.created_at = now;
    group.updated_at = now;

    let id = db.insert(group, GROUP).await?;
    let content = format!("<p>Group created with id: {}</p>", id);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

// #[delete("group")]
// async fn system_delete_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
//     let group = form.into_inner();
//     let _ = db
//         .delete(doc! {"name": &group.name}, GROUP, PhantomData::<Group>)
//         .await?;
//
//     let content = format!("<p>Group named {} has been deleted</p>", group.name);
//     Ok(HttpResponse::Ok()
//         .content_type(ContentType::form_url_encoded())
//         .body(content))
// }

#[patch("group")]
async fn system_update_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
    let group = form.into_inner();
    let _ = db
        .update(
            doc! {"name": &group.name},
            doc! {"$set": doc! {
                "name": &group.name,
                "subscriptions": &group.subscriptions,
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
            }},
            GROUP,
            PhantomData::<Group>,
        )
        .await?;

    let content = format!("<p>Group named {} has been updated</p>", group.name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[patch("user")]
async fn system_update_user(db: Data<&DB>, form: web::Form<User>) -> Result<HttpResponse> {
    let user = form.into_inner();
    let _ = db
        .update(
            doc! {"user_name": &user.user_name},
            doc! {"$set": doc! {
                "user_name": &user.user_name,
                "groups": &user.groups,
                "user_type": bson(&user.user_type)?,
                "updated_at": time::OffsetDateTime::now_utc().unix_timestamp(),
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

pub fn config_system(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/system_admin")
            .service(system)
            .service(system_create_group)
            // .service(system_delete_group)
            .service(system_update_group),
    );
}
