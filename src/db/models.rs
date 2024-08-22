use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utils::EnumToArrayStr;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, EnumToArrayStr)]
pub enum UserType {
    #[serde(rename = "user")]
    #[rename_variant = "user"]
    User,
    #[serde(rename = "group")]
    #[rename_variant = "group"]
    GroupAdmin,
    #[serde(rename = "system")]
    #[rename_variant = "system"]
    SystemAdmin,
}

// This is used in one of the deserialize.rs
impl From<&str> for UserType {
    fn from(s: &str) -> Self {
        match s {
            "user" => UserType::User,
            "group" => UserType::GroupAdmin,
            "system" => UserType::SystemAdmin,
            _ => UserType::User,
        }
    }
}

impl From<UserType> for &str {
    fn from(user_type: UserType) -> Self {
        match user_type {
            UserType::User => "user",
            UserType::GroupAdmin => "group",
            UserType::SystemAdmin => "system",
        }
    }
}

pub static USER: &str = "users";
#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub _id: ObjectId,
    pub sub: String,
    pub user_name: String,
    pub email: String,
    pub groups: Vec<String>,
    pub user_type: UserType,
    pub token: Option<String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub last_updated_by: String,
}

/// This is the form verison of the User struct
#[derive(Debug)]
pub struct UserForm {
    // sub is the safer way to identify a user, but sub is an arbitrary string if the user
    // registered through IBM ID. This would be not great on the admin portal. If we ensure the
    // email address has been verified, we can use that as the unique identifier.
    pub email: String,
    pub groups: Vec<String>,
    pub user_type: Option<UserType>,
    pub last_updated_by: String,
}

#[derive(Debug)]
pub struct UserDeleteForm {
    pub email: String,
    pub last_updated_by: String,
}

pub static GROUP: &str = "groups";
#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    pub _id: ObjectId,
    pub name: String,
    pub subscriptions: Vec<String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub last_updated_by: String,
}

/// This is the form verison of the Group struct
#[derive(Debug)]
pub struct GroupForm {
    pub name: String,
    pub subscriptions: Vec<String>,
    pub last_updated_by: String,
}

#[derive(Debug, Deserialize)]
pub struct UserGroupMod {
    pub email: String,
    pub modify_user: ModifyUser,
    pub group_name: String,
    pub last_updated_by: String,
}
#[derive(Debug, Deserialize)]
pub enum ModifyUser {
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "remove")]
    Remove,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GuardianCookie {
    pub subject: String,
    pub user_type: UserType,
}

#[derive(Debug, Deserialize, Clone)]
pub enum AdminTab {
    Profile,
    GroupCreate,
    GroupModify,
    UserModify,
    UserDelete,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AdminTabs {
    pub tab: AdminTab,
}

#[cfg(test)]
mod tests {
    use crate::db::models::UserType;

    #[test]
    fn test_usertype_partial_eq() {
        let user = UserType::User;
        let group = UserType::GroupAdmin;
        let system = UserType::SystemAdmin;

        assert_eq!(user, UserType::User);
        assert_eq!(group, UserType::GroupAdmin);
        assert_eq!(system, UserType::SystemAdmin);
    }

    #[test]
    fn test_enum_to_array() {
        let col = UserType::to_array_str();
        dbg!(&col);
        assert_eq!(col, ["user", "group", "system"])
    }
}
