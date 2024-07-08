use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum UserType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    GroupAdmin,
    #[serde(rename = "system")]
    SystemAdmin,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub sub: String,
    pub email: String,
    pub groups: Vec<String>,
    pub user_type: UserType,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Group {
    pub name: String,
    pub subscriptions: Vec<String>,
}
