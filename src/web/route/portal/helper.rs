use std::any::Any;

use crate::db::models::{Group, GuardianCookie, User};
use crate::errors::{GuardianError, Result};

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
