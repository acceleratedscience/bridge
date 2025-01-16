use std::str::FromStr;

use actix_web::{
    get,
    web::{Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

use crate::{
    db::{
        models::{BridgeCookie, Group, NotebookStatusCookie, User, UserType, GROUP, USER},
        mongo::DB,
        Database,
    },
    errors::{BridgeError, Result},
    web::{helper, route::portal::user_htmx::Profile},
};

const USER_PAGE: &str = "pages/portal_user.html";

#[get("user")]
#[instrument(skip(data, db, subject))]
pub(super) async fn user(
    data: Data<Tera>,
    req: HttpRequest,
    subject: Option<ReqData<BridgeCookie>>,
    nsc: Option<ReqData<NotebookStatusCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    if let Some(cookie_subject) = subject {
        let bridge_cookie = cookie_subject.into_inner();

        let id = ObjectId::from_str(&bridge_cookie.subject)
            .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

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
            Err(e) => return helper::log_with_level!(Err(e), error),
        };

        match user.user_type {
            UserType::User => {}
            _ => {
                return Err(BridgeError::UserNotAllowedOnPage(USER_PAGE.to_string()));
            }
        }

        // look up the subscriptions the group user belongs to
        let group: Result<Group> = db
            .find(
                doc! {"name": user.groups.first().unwrap_or(&"".into())},
                GROUP,
            )
            .await;

        let mut profile = Profile::new(&user);

        if let Ok(group) = group {
            user.groups.iter().for_each(|group| {
                profile.add_group(group.to_string());
            });
            group.subscriptions.iter().for_each(|subscription| {
                profile.add_subscription(subscription.to_string());
            });
        }
        let content = helper::log_with_level!(
            profile
                .render(data, nsc, bridge_cookie, helper::add_token_exp_to_tera)
                .await,
            error
        )?;

        if let Some([nc, nsc, bc]) = content.1 {
            return Ok(HttpResponse::Ok()
                .cookie(nc)
                .cookie(nsc)
                .cookie(bc)
                .body(content.0));
        }

        return Ok(HttpResponse::Ok().body(content.0));
    }

    helper::log_with_level!(
        Err(BridgeError::UserNotFound(
            "subject not passed from middleware".to_string(),
        )),
        error
    )
}
