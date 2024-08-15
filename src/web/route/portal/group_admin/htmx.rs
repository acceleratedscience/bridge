use actix_web::web::Data;
use tera::Tera;

use crate::errors::Result;
use crate::web::route::portal::PROFILE_MAIN;

pub(super) static MODIFY_USER_GROUP: &str = "components/user_edit.html";

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

pub struct Profile {
	pub groups: Vec<String>,
	pub subscriptions: Vec<String>,
}

impl Profile {
	pub fn new() -> Self {
		Self {
			groups: Vec::new(),
			subscriptions: Vec::new(),
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

		Ok(tera.render(PROFILE_MAIN, &context)?)
	}
}
