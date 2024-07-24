use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;

pub(super) static MODIFY_USER_GROUP: &str = "group/modify_user.html";

pub struct ModifyUserGroup {
    user_in_group: Vec<String>,
}

impl ModifyUserGroup {
    pub fn new() -> Self {
        Self {
            user_in_group: Vec::new(),
        }
    }

    pub fn add(&mut self, item: String) {
        self.user_in_group.push(item);
    }

    pub fn render(&self, subject: &str, group_name: &str, tera: Data<Tera>) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("subject", &subject);
        context.insert("members", &self.user_in_group.join(", "));
        context.insert("group_name", group_name);

        Ok(tera.render(MODIFY_USER_GROUP, &context)?)
    }
}
