use std::{fs::read_to_string, path::PathBuf, str::FromStr, sync::OnceLock};

use toml::Value;
use url::Url;

use crate::errors::{GuardianError, Result};

pub struct Catalog(pub toml::Table);

pub static CATALOG: OnceLock<Catalog> = OnceLock::new();
pub static CATALOG_URLS: OnceLock<Vec<(Url, String)>> = OnceLock::new();

pub fn init_once() {
	let table = toml::from_str(
		&read_to_string(PathBuf::from_str("config/services.toml").unwrap()).unwrap(),
	)
	.unwrap();
	CATALOG.get_or_init(|| Catalog(table));
	CATALOG_URLS.get_or_init(|| CATALOG.get().unwrap().into());
}

impl Catalog {
	pub fn get(&self, service_name: &str) -> Result<Url> {
		let catalog = self.0.get("services").ok_or_else(|| {
			GuardianError::GeneralError("services definition not found in config".to_string())
		})?;
		let service = catalog
			.get(service_name)
			.ok_or_else(|| GuardianError::ServiceDoesNotExist(service_name.to_string()))?;
		let url = service.get("url").ok_or_else(|| {
			GuardianError::GeneralError("url not found in service definition".to_string())
		})?;

		Url::parse(
			url.as_str()
				.ok_or_else(|| GuardianError::GeneralError("url not a string".to_string()))?,
		)
		.map_err(|e| GuardianError::GeneralError(e.to_string()))
	}
}

impl From<&Catalog> for Vec<(Url, String)> {
	fn from(value: &Catalog) -> Self {
		if let Some(map) = value.0.get("services").and_then(|v| v.as_table()) {
			return map
				.iter()
				.filter_map(|(name, service)| {
					let health_endpoint = service
						.get("readiness")
						.and_then(Value::as_str)
						.unwrap_or("health");

					let url = service
						.get("url")
						.and_then(Value::as_str)
						.and_then(|url| Url::parse(url).ok())
						.and_then(|url| url.join(health_endpoint).ok());
					url.map(|url| (url, name.to_string()))
				})
				.collect();
		}
		vec![]
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_catalog() {
		init_once();
		let catalog = CATALOG.get().unwrap();
		let service = catalog.get("postman").unwrap();
		assert_eq!(service.as_str(), "https://postman-echo.com/");
	}

	#[test]
	fn test_catalog_into() {
		init_once();
		let catalog = CATALOG.get().unwrap();
		let services: Vec<(Url, String)> = catalog.into();
		assert_eq!(services.len(), 6);

		let postman = services.iter().find(|(_, name)| name == "postman");
		assert!(postman.is_some());
	}
}
