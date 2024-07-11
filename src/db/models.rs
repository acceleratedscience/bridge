use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
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
}

pub static GROUP: &str = "groups";
#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    _id: ObjectId,
    pub name: String,
    pub subscriptions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GuardianCookie {
    pub subject: String,
    pub user_type: UserType,
}
