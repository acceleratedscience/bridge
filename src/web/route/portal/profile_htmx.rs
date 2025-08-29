#![allow(dead_code)]
use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;

pub struct Profile {
    pub groups: Vec<String>,
    pub subscriptions: Vec<String>,
    pub name: String,
}

pub(super) static PROFILE: &str = "components/token.html";

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

        context.insert("group", &self.groups.join(", "));
        context.insert("subscriptions", &subs_expanded);
        context.insert("name", &self.name);

        Ok(tera.render(PROFILE, &context)?)
    }
}
