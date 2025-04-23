use mongodb::bson;
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
// User struct is the Rust representation of the user collection in the database
#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub _id: ObjectId,
    pub sub: String,
    pub user_name: String,
    pub email: String,
    pub groups: Vec<String>,
    pub user_type: UserType,
    pub token: Option<String>,
    pub notebook: Option<NotebookInfo>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub last_updated_by: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserPortalRep {
    pub _id: String,
    pub sub: String,
    pub user_name: String,
    pub email: String,
    pub user_type: &'static str,
    pub created_at: String,
    pub updated_at: String,
    pub last_updated_by: String,
}

impl From<User> for UserPortalRep {
    fn from(user: User) -> Self {
        UserPortalRep {
            _id: user._id.to_string(),
            sub: user.sub,
            user_name: user.user_name,
            email: user.email,
            user_type: user.user_type.into(),
            created_at: user.created_at.to_string(),
            updated_at: user.updated_at.to_string(),
            last_updated_by: user.last_updated_by,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct NotebookInfo {
    pub start_time: Option<time::OffsetDateTime>,
    pub last_active: Option<time::OffsetDateTime>,
    pub max_idle_time: Option<u64>,
    pub start_up_url: Option<String>,
    pub persist_pvc: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserNotebook {
    pub name: String,
    pub url: String,
    pub start_time: String,
    pub status: String,
    pub start_up_url: Option<String>,
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
// Group struct is the Rust representation of the group collection in the database
#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    pub _id: ObjectId,
    pub name: String,
    pub subscriptions: Vec<String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub last_updated_by: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GroupSubs {
    pub _id: ObjectId,
    pub group_id: Vec<ObjectId>,
    pub group_name: Vec<String>,
    pub group_subscriptions: Vec<Vec<String>>,
}

pub static LOCKS: &str = "locks";
#[derive(Debug, Deserialize, Serialize)]
pub struct Locks {
    pub _id: ObjectId,
    #[serde(rename = "leaseName")]
    pub lease_name: String,
    #[serde(rename = "expireSoonAfter")]
    pub expire_soon_after: bson::DateTime,
}

pub static APPS: &str = "applications";
#[derive(Debug, Deserialize, Serialize)]
pub struct Apps {
    pub client_id: String,
    pub client_secret: String,
    pub salt: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct AppPayload {
    pub username: String,
    pub password: String,
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
pub struct BridgeCookie {
    pub subject: String,
    pub user_type: UserType,
    pub config: Option<Config>,
    pub resources: Option<Vec<String>>,
    pub token: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub notebook_persist_pvc: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NotebookCookie {
    /// The name of the subject the owns notebook CRD
    pub subject: String,
    pub ip: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NotebookStatusCookie {
    pub start_time: String,
    pub status: String,
    pub start_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub enum AdminTab {
    Profile,
    GroupView,
    GroupCreate,
    GroupModify,
    UserView,
    UserModify,
    UserDelete,
    Main,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AdminTabs {
    pub tab: AdminTab,
    pub user: Option<String>,
    pub group: Option<String>,
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
    fn test_bridge_cookie_serde() {
        let bridge_cookie =
            r#"{"subject":"test","user_type":"system","config":null,"resources":null}"#;
        let bridge_cookie: super::BridgeCookie = serde_json::from_str(bridge_cookie).unwrap();
        assert_eq!(bridge_cookie.subject, "test");
        assert_eq!(bridge_cookie.user_type, UserType::SystemAdmin);
        assert!(bridge_cookie.config.is_none());
        assert!(bridge_cookie.resources.is_none());
    }

    #[test]
    fn test_enum_to_array() {
        let col = UserType::to_array_str();
        dbg!(&col);
        assert_eq!(col, ["user", "group", "system"])
    }
}
