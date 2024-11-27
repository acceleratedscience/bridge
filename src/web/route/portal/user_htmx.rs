use actix_web::web::Data;
use tera::{Context, Tera};

use crate::db::models::{User, UserNotebook};
use crate::errors::Result;

pub struct Profile<'p> {
    pub groups: Vec<String>,
    pub subscriptions: Vec<String>,
    user: &'p User,
}

pub(super) static PROFILE: &str = "pages/portal_user.html";

impl<'p> Profile<'p> {
    pub fn new(user: &'p User) -> Self {
        Self {
            groups: Vec::new(),
            subscriptions: Vec::new(),
            user,
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
        context.insert("name", &self.user.user_name);
        context.insert("token", &self.user.token);

        // add in the expiration time if token is present
        if let Some(t) = &self.user.token {
            t_exp(&mut context, t);
        }
        if self.subscriptions.contains(&"notebook".to_string()) {
            context.insert("notebook", &Into::<UserNotebook>::into(self.user));
        }

        Ok(tera.render(PROFILE, &context)?)
    }
}
