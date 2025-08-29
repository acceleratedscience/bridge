use actix_web::{
    cookie::Cookie,
    web::{Data, ReqData},
};
use tera::{Context, Tera};

use crate::{
    db::models::{BridgeCookie, NotebookStatusCookie, OWUICookie, User},
    errors::Result,
    web::services::CATALOG,
};

#[cfg(feature = "notebook")]
use super::helper::notebook_bookkeeping;

pub struct Profile<'p> {
    pub groups: Vec<String>,
    pub subscriptions: Vec<String>,
    user: &'p User,
}

pub(super) static PROFILE: &str = "pages/portal_user.html";
pub(super) static EMPTY_PROFILE: &str = "pages/pending_user.html";

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

    #[allow(unused_variables)]
    pub async fn render(
        &self,
        tera: Data<Tera>,
        context: Data<Context>,
        nsc: Option<ReqData<NotebookStatusCookie>>,
        oc: Option<ReqData<OWUICookie>>,
        bc: &mut BridgeCookie,
        t_exp: impl FnOnce(&mut Context, &str),
    ) -> Result<(String, Option<[Cookie<'_>; 2]>)> {
        let mut context = (**context).clone();
        context.insert("name", &self.user.user_name);

        if self.groups.is_empty() {
            context.insert("user_type", "pending");
            return Ok((tera.render(EMPTY_PROFILE, &context)?, None));
        }

        // Placeholder: need to fetch subscription parameters from services.toml
        let subs_expanded: Vec<serde_json::Value> = self.subscriptions
            .iter()
            .map(|service_name| {
                serde_json::json!({
                    "type": "openad_model",
                    "url": "https://dummy.url",
                    "name": service_name,
                    "nickname": &service_name.strip_prefix("mcp-").unwrap_or(service_name)[0..4],
                })
            })
            .collect();

        context.insert("user_type", &self.user.user_type);
        context.insert("email", &self.user.email);
        context.insert("group", &self.groups);
        context.insert("subscriptions", &subs_expanded);
        context.insert("token", &self.user.token);
        // add in the expiration time if token is present
        if let Some(t) = &self.user.token {
            t_exp(&mut context, t);
        }

        if let Some(ref resources) = bc.resources {
            let resources: Vec<(&String, bool)> = resources
                .iter()
                .map(|r| {
                    let show = CATALOG
                        .get_details("resources", r, "show")
                        .map(|v| v.as_bool().unwrap_or(false));
                    (r, show.unwrap_or(false))
                })
                .collect();
            context.insert("resources", &resources);
        }

        #[cfg(feature = "openwebui")]
        if let Some(owui_cookie) = oc {
            use crate::config::CONFIG;

            context.insert("openwebui", &owui_cookie.subject);
            context.insert("owui_url", &CONFIG.openweb_url);
        }

        #[cfg(feature = "notebook")]
        let nb_cookies =
            notebook_bookkeeping(self.user, nsc, bc, &mut context, self.subscriptions.clone())
                .await?;

        // TODO: with the notebook flow refactor '25... this doesn't exactly fit anymore... right now
        // it should not affect the functionality of the application for keeping this around.  But
        // remove or redo this later.
        #[cfg(feature = "notebook")]
        if let Some(ref conf) = bc.config {
            context.insert("pvc", &conf.notebook_persist_pvc);
        }

        #[cfg(feature = "notebook")]
        return Ok((tera.render(PROFILE, &context)?, nb_cookies));

        #[cfg(not(feature = "notebook"))]
        Ok((tera.render(PROFILE, &context)?, None))
    }
}
