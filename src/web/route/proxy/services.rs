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
    LazyLock::new(|| Into::<ServiceCatalog>::into(LazyLock::force(&CATALOG)).into());
static CATALOG_ALL_NAMES: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut names = vec![];
    names.extend(
        LazyLock::force(&CATALOG)
            .0
            .get("services")
            .and_then(|v| v.as_table())
            .expect("services not found in config")
            .keys()
            .map(|k| k.to_string()),
    );
    names.extend(
        LazyLock::force(&CATALOG)
            .0
            .get("resources")
            .and_then(|v| v.as_table())
            .expect("resources not found in config")
            .keys()
            .map(|k| k.to_string()),
    );
    names
});
static ALL_RESOURCE_NAMES: LazyLock<Vec<&str>> = LazyLock::new(|| {
    LazyLock::force(&CATALOG)
        .0
        .get("resources")
        .and_then(|v| v.as_table())
        .expect("resources not found in config")
        .keys()
        .map(|k| k.as_ref())
        .collect()
});

impl Catalog {
    #[inline]
    fn get_inner(&self, type_: &str, name: &str) -> Result<Url> {
        let catalog = self.0.get(type_).ok_or_else(|| {
            BridgeError::GeneralError("services definition not found in config".to_string())
        })?;
        let service = catalog
            .get(name)
            .ok_or_else(|| BridgeError::ServiceDoesNotExist(name.to_string()))?;
        let url = service.get("url").ok_or_else(|| {
            BridgeError::GeneralError("url not found in service definition".to_string())
        })?;

        Url::parse(
            url.as_str()
                .ok_or_else(|| BridgeError::GeneralError("url not a string".to_string()))?,
        )
        .map_err(|e| BridgeError::GeneralError(e.to_string()))
    }

    pub fn get_service(&self, service_name: &str) -> Result<Url> {
        self.get_inner("services", service_name)
    }

    pub fn get_resource(&self, resource_name: &str) -> Result<Url> {
        self.get_inner("resources", resource_name)
    }

    pub fn get_all_resources_by_name(&self) -> &'static Vec<&str> {
        &ALL_RESOURCE_NAMES
    }

    pub fn get_all_by_name(&self) -> &'static Vec<String> {
        &CATALOG_ALL_NAMES
    }
}

pub struct ResourceCatalog(Vec<(Url, String)>);
impl From<ResourceCatalog> for Vec<(Url, String)> {
    fn from(value: ResourceCatalog) -> Self {
        value.0
    }
}
pub struct ServiceCatalog(Vec<(Url, String)>);
impl From<ServiceCatalog> for Vec<(Url, String)> {
    fn from(value: ServiceCatalog) -> Self {
        value.0
    }
}

// For services
impl From<&Catalog> for ServiceCatalog {
    fn from(value: &Catalog) -> Self {
        Self(match value.0.get("services").and_then(|v| v.as_table()) {
            Some(map) => {
                map.iter()
                    .filter_map(|(name, service)| {
                        // In the services.toml, there are entries that are not services with health
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
                    .collect()
            }
            None => vec![],
        })
    }
}

impl From<&Catalog> for ResourceCatalog {
    fn from(value: &Catalog) -> Self {
        Self(match value.0.get("resources").and_then(|v| v.as_table()) {
            Some(map) => map
                .iter()
                .filter_map(|(name, service)| {
                    let url = service
                        .get("url")
                        .and_then(Value::as_str)
                        .and_then(|url| Url::parse(url).ok());
                    url.map(|url| (url, name.to_string()))
                })
                .collect(),
            None => vec![],
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_catalog() {
        let catalog = &CATALOG;
        let service = catalog.get_service("postman").unwrap();
        assert_eq!(service.as_str(), "https://postman-echo.com/");

        let resource = catalog.get_resource("reddit").unwrap();
        assert_eq!(resource.as_str(), "https://www.reddit.com/");

        let service = catalog.get_service("notebook");
        assert!(service.is_err());
    }

    #[test]
    fn test_catalog_into() {
        let services: Vec<(Url, String)> =
            Into::<ServiceCatalog>::into(LazyLock::force(&CATALOG)).into();
        assert_eq!(services.len(), 7);

        let resources: Vec<(Url, String)> =
            Into::<ResourceCatalog>::into(LazyLock::force(&CATALOG)).into();
        assert!(resources.len().ge(&2));

        let postman = services.iter().find(|(_, name)| name == "postman");
        assert!(postman.is_some());
    }

    #[test]
    fn test_catalog_all_names() {
        let names = CATALOG.get_all_by_name();
        assert!(names.contains(&"postman".to_string()));
        assert!(names.contains(&"reddit".to_string()));
    }
}
