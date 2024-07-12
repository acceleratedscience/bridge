use std::{marker::PhantomData, str::FromStr};

use actix_web::web;
use actix_web::{
    delete, get,
    http::header::ContentType,
    patch, post,
    web::{Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

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

#[get("system_admin")]
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

    let id = db.insert(group, GROUP).await?;
    let content = format!("<p>Group created with id: {}</p>", id);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[delete("group")]
async fn system_delete_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
    let group = form.into_inner();
    let _ = db
        .delete(doc! {"name": &group.name}, GROUP, PhantomData::<Group>)
        .await?;
    let content = format!("<p>Group named {} has been deleted</p>", group.name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[patch("group")]
async fn system_update_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
    // let group = form.into_inner();
    // let id = db
    //     .update(
    //         doc! {"name": &group.name},
    //         doc! {"$set": doc! {
    //             "name": &group.name,
    //             "subscriptions": &group.subscriptions,
    //             "updated_at": sec_since_unix_epoch()?,
    //         }},
    //         GROUP,
    //         PhantomData::<Group>,
    //     )
    //     .await;
    Ok(HttpResponse::Ok().finish())
}
