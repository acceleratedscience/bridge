use std::{marker::PhantomData, str::FromStr};

use actix_web::{
    get,
    http::header::ContentType,
    web::{Data, ReqData},
    HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;

use crate::{
    auth::jwt,
    config::{AUD, CONFIG},
    db::{
        models::{Group, BridgeCookie, User, GROUP, USER},
        mongo::DB,
        Database,
    },
    errors::{BridgeError, Result},
    web::helper::{self, bson},
};

const TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };

#[get("token")]
pub async fn get_token_for_user(
    subject: Option<ReqData<BridgeCookie>>,
    data: Data<Tera>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    let gc = match subject {
        Some(cookie_subject) => cookie_subject.into_inner(),
        None => {
            return helper::log_with_level!(
                Err(BridgeError::UserNotFound(
                    "subject not passed from middleware".to_string(),
                )),
                error
            )
        }
    };
    let id =
        ObjectId::from_str(&gc.subject).map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    // get information about user
    let user: User = helper::log_with_level!(
        db.find(
            doc! {
                "_id": id,
            },
            USER,
        )
        .await,
        error
    )?;

    let scp = if user.groups.is_empty() {
        vec!["".to_string()]
    } else {
        // get models
        let group: Group = helper::log_with_level!(
            db.find(
                doc! {
                    "name": &user.groups[0]
                },
                GROUP,
            )
            .await,
            error
        )?;
        group.subscriptions
    };

    // Generate bridge token
    let (token, exp) = helper::log_with_level!(
        jwt::get_token_and_exp(&CONFIG.encoder, TOKEN_LIFETIME, &gc.subject, AUD[0], scp),
        error
    )?;

    // store thew newly create token in the database
    let r = helper::log_with_level!(
        db.update(
            doc! {
                "_id": id,
            },
            doc! {"$set": doc! {
            "updated_at": bson(time::OffsetDateTime::now_utc())?,
            "token": &token,
            "last_updated_by": user.email }},
            USER,
            PhantomData::<User>,
        )
        .await,
        error
    )?;

    if r.ne(&1) {
        return helper::log_with_level!(
            Err(BridgeError::GeneralError("Token not updated".to_string())),
            error
        );
    }

    let mut context = tera::Context::new();
    context.insert("token", &Some(token));
    context.insert("token_exp", &exp);
    let content = helper::log_with_level!(data.render("components/token.html", &context), error)?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}
