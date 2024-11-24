use actix_web::web::Data;
use tera::{Context, Tera};

use crate::errors::Result;

pub struct Profile {
    pub groups: Vec<String>,
    pub subscriptions: Vec<String>,
    pub name: String,
    pub token: Option<String>,
}

pub(super) static PROFILE: &str = "pages/portal_user.html";

impl Profile {
    pub fn new(name: String, token: Option<String>) -> Self {
        Self {
            groups: Vec::new(),
            subscriptions: Vec::new(),
            name,
            token,
        }
    }

    pub fn add_group(&mut self, group: String) {
        self.groups.push(group);
    }

    pub fn add_subscription(&mut self, subscription: String) {
        self.subscriptions.push(subscription);
    }

    pub fn render(
        &self,
        tera: Data<Tera>,
        t_exp: impl FnOnce(&mut Context, &str),
    ) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("group", &self.groups.join(", "));
        context.insert("subscriptions", &self.subscriptions);
        context.insert("name", &self.name);
        context.insert("token", &self.token);

        // add in the expiration time if token is present
        if let Some(t) = &self.token {
            t_exp(&mut context, t);
        }
        if self.subscriptions.contains(&"notebook".to_string()) {
            context.insert("notebook", &true);
        }

        Ok(tera.render(PROFILE, &context)?)
    }
}
