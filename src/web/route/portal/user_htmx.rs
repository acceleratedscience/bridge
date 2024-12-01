use actix_web::{
    cookie::Cookie,
    web::{Data, ReqData},
};
use tera::{Context, Tera};

use crate::{
    db::models::{NotebookStatusCookie, User},
    errors::Result,
};

use super::helper::notebook_bookkeeping;

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

    pub async fn render(
        &self,
        tera: Data<Tera>,
        nsc: Option<ReqData<NotebookStatusCookie>>,
        t_exp: impl FnOnce(&mut Context, &str),
    ) -> Result<(String, Option<[Cookie; 2]>)> {
        let mut context = tera::Context::new();
        context.insert("group", &self.groups.join(", "));
        context.insert("subscriptions", &self.subscriptions);
        context.insert("name", &self.user.user_name);
        context.insert("token", &self.user.token);

        // add in the expiration time if token is present
        if let Some(t) = &self.user.token {
            t_exp(&mut context, t);
        }
        let nb_cookies =
            notebook_bookkeeping(self.user, nsc, &mut context, self.subscriptions.clone()).await?;

        Ok((tera.render(PROFILE, &context)?, nb_cookies))
    }
}
