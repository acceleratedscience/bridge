use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;

pub struct Profile {
    pub groups: Vec<String>,
    pub subscriptions: Vec<String>,
    pub name: String,
}

pub(super) static PROFILE: &str = "user/user.html";

impl Profile {
    pub fn new(name: String) -> Self {
        Self {
            groups: Vec::new(),
            subscriptions: Vec::new(),
            name,
        }
    }

    pub fn add_group(&mut self, group: String) {
        self.groups.push(group);
    }

    pub fn add_subscription(&mut self, subscription: String) {
        self.subscriptions.push(subscription);
    }

    pub fn render(&self, tera: Data<Tera>) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("group", &self.groups.join(", "));
        context.insert("subscriptions", &self.subscriptions);
        context.insert("name", &self.name);

        Ok(tera.render(PROFILE, &context)?)
    }
}
