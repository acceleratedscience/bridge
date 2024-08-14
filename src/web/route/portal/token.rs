use std::str::FromStr;

use actix_web::{
    get,
    http::header::ContentType,
    web::{Data, ReqData},
    HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};

use crate::{
    auth::jwt,
    config::{AUD, CONFIG},
    db::{
        models::{Group, GuardianCookie, User, GROUP, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper,
};

const TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };

#[get("token")]
pub async fn get_token_for_user(
    subject: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    let gc = match subject {
        Some(cookie_subject) => cookie_subject.into_inner(),
        None => {
            return helper::log_errors(Err(GuardianError::UserNotFound(
                "subject not passed from middleware".to_string(),
            )))
        }
    };
    let id = ObjectId::from_str(&gc.subject)
        .map_err(|e| GuardianError::GeneralError(e.to_string()))?;

    // get information about user
    let user: User = helper::log_errors(
        db.find(
            doc! {
                "_id": id,
            },
            USER,
        )
        .await,
    )?;

    let scp = if user.groups.is_empty() {
        vec!["".to_string()]
    } else {
        // get models
        let group: Group = helper::log_errors(
            db.find(
                doc! {
                    "name": &user.groups[0]
                },
                GROUP,
            )
            .await,
        )?;
        group.subscriptions
    };

    // Generate guardian token
    let token = helper::log_errors(jwt::get_token(
        &CONFIG.encoder,
        TOKEN_LIFETIME,
        &gc.subject,
        AUD[0],
        scp,
    ))?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(token))
}
