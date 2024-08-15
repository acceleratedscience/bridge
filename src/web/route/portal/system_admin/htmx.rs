use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;

pub struct GroupContent {
    items: Vec<String>,
}

pub(super) static CREATE_GROUP: &str = "components/group_edit.html";
pub(super) static MODIFY_GROUP: &str = "components/group_edit.html";

impl GroupContent {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: String) {
        self.items.push(item);
    }

    pub fn render(&self, subject: &str, tera: Data<Tera>, template_name: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("subject", &subject);
        context.insert("items", &self.items);

        Ok(tera.render(template_name, &context)?)
    }
}

pub struct UserContent {
    group_items: Vec<String>,
    user_items: Vec<String>,
}

pub(super) static DELETE_USER: &str = "components/user_edit.html"; // DCH, no need for separate page.
pub(super) static MODIFY_USER: &str = "components/user_edit.html";

impl UserContent {
    pub fn new() -> Self {
        Self {
            group_items: Vec::new(),
            user_items: Vec::new(),
        }
    }

    pub fn add_group(&mut self, item: String) {
        self.group_items.push(item);
    }

    pub fn add_user(&mut self, item: String) {
        self.user_items.push(item);
    }

    pub fn render(&self, subject: &str, tera: Data<Tera>, template_name: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("subject", &subject);
        context.insert("group_items", &self.group_items);
        context.insert("user_items", &self.user_items);

        Ok(tera.render(template_name, &context)?)
    }
}
