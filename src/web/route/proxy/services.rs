use std::sync::LazyLock;
use std::{fs::read_to_string, path::PathBuf, str::FromStr};

use toml::Value;
use url::Url;

use crate::errors::{BridgeError, Result};

pub struct Catalog(pub toml::Table);

pub static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    Catalog(
        toml::from_str(
            &read_to_string(PathBuf::from_str("config/services.toml").unwrap()).unwrap(),
        )
        .unwrap(),
    )
});
pub static CATALOG_URLS: LazyLock<Vec<(Url, String)>> =
    LazyLock::new(|| LazyLock::force(&CATALOG).into());

impl Catalog {
    pub fn get(&self, service_name: &str) -> Result<Url> {
        let catalog = self.0.get("services").ok_or_else(|| {
            BridgeError::GeneralError("services definition not found in config".to_string())
        })?;
        let service = catalog
            .get(service_name)
            .ok_or_else(|| BridgeError::ServiceDoesNotExist(service_name.to_string()))?;
        let url = service.get("url").ok_or_else(|| {
            BridgeError::GeneralError("url not found in service definition".to_string())
        })?;

        Url::parse(
            url.as_str()
                .ok_or_else(|| BridgeError::GeneralError("url not a string".to_string()))?,
        )
        .map_err(|e| BridgeError::GeneralError(e.to_string()))
    }
}

impl From<&Catalog> for Vec<(Url, String)> {
    fn from(value: &Catalog) -> Self {
        if let Some(map) = value.0.get("services").and_then(|v| v.as_table()) {
            return map
                .iter()
                .filter_map(|(name, service)| {
                    // In the service.toml, there are entries that are not services with health
                    // endpoints, such as notebooks. We need to filter them out.
                    if !service
                        .get("check")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        return None;
                    }

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
        let catalog = &CATALOG;
        let service = catalog.get("postman").unwrap();
        assert_eq!(service.as_str(), "https://postman-echo.com/");

        let service = catalog.get("notebook");
        assert!(service.is_err());
    }

    #[test]
    fn test_catalog_into() {
        let catalog = &CATALOG;
        let services: Vec<(Url, String)> = LazyLock::force(catalog).into();
        assert_eq!(services.len(), 7);

        let postman = services.iter().find(|(_, name)| name == "postman");
        assert!(postman.is_some());
    }
}
