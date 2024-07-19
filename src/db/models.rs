use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum UserType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    GroupAdmin,
    #[serde(rename = "system")]
    SystemAdmin,
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
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
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

#[derive(Debug)]
pub struct GroupForm {
    pub name: String,
    pub subscriptions: Vec<String>,
    pub last_updated_by: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GuardianCookie {
    pub subject: String,
    pub user_type: UserType,
}

#[derive(Debug, Deserialize, Clone)]
pub enum AdminTab {
    Profile,
    Group,
    User,
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
}
