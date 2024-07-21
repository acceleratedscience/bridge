use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;

pub struct GroupContent {
    items: Vec<String>,
}

pub(super) static CREATE: &str = "system/create_group.html";
pub(super) static MODIFY: &str = "system/modify_group.html";

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
