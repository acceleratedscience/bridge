use std::any::Any;

use actix_web::web::ReqData;
use mongodb::bson::doc;

use crate::{
    db::{
        models::{Group, GuardianCookie, User, UserType, GROUP},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper,
};

#[allow(dead_code)]
#[allow(unused_variables)]
pub(super) fn portal_hygienic_group(gc: &GuardianCookie, doc: &dyn Any) -> Result<bool> {
    // Downcast to Group
    if let Some(group) = doc.downcast_ref::<Group>() {
        // group_admin cannot edit groups
        return Ok(false);
    }

    if let Some(user) = doc.downcast_ref::<User>() {
        // if user.groups.contains(gc.user_type)
        return Ok(true);
    }

    Err(GuardianError::GeneralError(
        "Portal Hygienic Error".to_string(),
    ))
}

pub(super) fn check_admin(
    subject: Option<ReqData<GuardianCookie>>,
    admin: UserType,
) -> Result<GuardianCookie> {
    Ok(match subject {
        // System admin
        Some(cookie_subject) if cookie_subject.user_type == admin => cookie_subject.into_inner(),
        // All other users
        Some(g) => {
            return helper::log_with_level!(
                Err(GuardianError::UserNotFound(format!(
                    "User {} is not a system admin",
                    g.into_inner().subject
                ))),
                error
            )
        }
        None => {
            return helper::log_with_level!(
                Err(GuardianError::UserNotFound(
                    "No user passed from middleware... subject not passed from middleware"
                        .to_string(),
                )),
                error
            )
        }
    })
}

/// This is a helper function to get all Groups from the database
pub(super) async fn get_all_groups(db: &DB) -> Result<Vec<Group>> {
    let result: Result<Vec<Group>> = db.find_many(doc! {}, GROUP).await;
    Ok(match result {
        Ok(groups) => groups,
        Err(e) => return helper::log_with_level!(Err(e), warn),
    })
}
