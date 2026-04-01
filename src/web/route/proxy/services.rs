use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::{env, fs::read_to_string, path::PathBuf, str::FromStr};

use parking_lot::RwLock;
use toml::Value;
use url::Url;

use crate::errors::{BridgeError, Result};

const SERVICES_CONFIG_PATH_ENV: &str = "BRIDGE_SERVICES_CONFIG_PATH";

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub kind: String,
    pub mcp: bool,
    pub description: String,
}

#[derive(Default)]
struct CatalogState {
    catalog: toml::Table,
    all: Arc<HashMap<String, CatalogEntry>>,
    all_resource_names: Arc<Vec<String>>,
    health_urls: Arc<Vec<(Url, String)>>,
}

impl CatalogState {
    fn from_catalog(catalog: toml::Table) -> Self {
        let mut all = HashMap::new();
        let mut all_resource_names = Vec::new();

        if let Some(services) = catalog.get("services").and_then(Value::as_table) {
            for (name, service) in services {
                let mcp = service
                    .get("mcp")
                    .and_then(Value::as_bool)
                    .unwrap_or_default();
                let description = service
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                all.insert(
                    name.to_string(),
                    CatalogEntry {
                        kind: "service".to_string(),
                        mcp,
                        description,
                    },
                );
            }
        }

        if let Some(resources) = catalog.get("resources").and_then(Value::as_table) {
            for (name, resource) in resources {
                let description = resource
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                all.insert(
                    name.to_string(),
                    CatalogEntry {
                        kind: "resource".to_string(),
                        mcp: false,
                        description,
                    },
                );
                all_resource_names.push(name.to_string());
            }
        }

        let health_urls: Vec<(Url, String)> = catalog
            .get("services")
            .and_then(Value::as_table)
            .map(|services| {
                services
                    .iter()
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
            })
            .unwrap_or_default();

        Self {
            catalog,
            all: Arc::new(all),
            all_resource_names: Arc::new(all_resource_names),
            health_urls: Arc::new(health_urls),
        }
    }
}

// TODO: move this out of proxy mod... perhaps in the parent mod to this

static CATALOG_STATE: LazyLock<RwLock<CatalogState>> =
    LazyLock::new(|| RwLock::new(CatalogState::default()));

fn default_service_config_path() -> &'static str {
    if cfg!(debug_assertions) {
        "config/services_sample.toml"
    } else {
        "config/services.toml"
    }
}

fn current_service_config_path() -> String {
    env::var(SERVICES_CONFIG_PATH_ENV).unwrap_or_else(|_| default_service_config_path().to_string())
}

fn load_from_path(path: &str) -> Result<CatalogState> {
    let catalog: toml::Table =
        toml::from_str(&read_to_string(PathBuf::from_str(path).map_err(|e| {
            BridgeError::GeneralError(format!("Invalid config path '{path}': {e}"))
        })?)?)?;

    Ok(CatalogState::from_catalog(catalog))
}

pub fn init_once() -> Result<()> {
    reload()
}

pub fn reload() -> Result<()> {
    let path = current_service_config_path();
    let next_state = load_from_path(&path)?;
    let mut guard = CATALOG_STATE.write();
    *guard = next_state;
    Ok(())
}

#[inline]
fn get_inner(type_: &str, name: &str) -> Result<Url> {
    let guard = CATALOG_STATE.read();
    let catalog = guard.catalog.get(type_).ok_or_else(|| {
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

pub fn get_service(service_name: &str) -> Result<Url> {
    get_inner("services", service_name)
}

#[cfg(feature = "mcp")]
pub fn is_service_mcp(service_name: &str) -> Result<bool> {
    let guard = CATALOG_STATE.read();
    Ok(guard
        .catalog
        .get("services")
        .ok_or_else(|| {
            BridgeError::GeneralError("services definition not found in config".to_string())
        })?
        .get(service_name)
        .ok_or_else(|| BridgeError::ServiceDoesNotExist(service_name.to_string()))?
        .get("mcp")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

pub fn get_resource(resource_name: &str) -> Result<Url> {
    get_inner("resources", resource_name)
}

pub fn get_detail(type_: &str, name: &str, field: &str) -> Option<Value> {
    let guard = CATALOG_STATE.read();
    guard.catalog.get(type_)?.get(name)?.get(field).cloned()
}

pub fn get_all_resources_by_name() -> Arc<Vec<String>> {
    CATALOG_STATE.read().all_resource_names.clone()
}

// get all service and resources by their (in this order) name, kind, whether or not mcp, and description
pub fn get_all() -> Arc<HashMap<String, CatalogEntry>> {
    CATALOG_STATE.read().all.clone()
}

pub fn get_service_health_urls() -> Arc<Vec<(Url, String)>> {
    CATALOG_STATE.read().health_urls.clone()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_catalog() {
        init_once().unwrap();
        let service = get_service("postman").unwrap();
        assert_eq!(service.as_str(), "https://postman-echo.com/");

        let resource = get_resource("example").unwrap();
        assert_eq!(resource.as_str(), "https://www.example.com/");

        let service = get_service("notebook");
        assert!(service.is_err());
    }

    #[test]
    fn test_catalog_health_urls() {
        init_once().unwrap();
        let services = get_service_health_urls();
        assert_ne!(services.len(), 0);

        let postman = services.iter().find(|(_, name)| name == "postman");
        assert!(postman.is_some());
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn test_mcp_bool() {
        init_once().unwrap();
        let mcp = is_service_mcp("postman").unwrap();
        assert!(!mcp);
    }

    #[test]
    fn test_catalog_all_names() {
        init_once().unwrap();
        let names = get_all();
        assert!(names.len() >= 2);
    }

    #[test]
    fn test_get_details() {
        init_once().unwrap();
        let Value::Boolean(b) = get_detail("resources", "example", "show").unwrap() else {
            panic!("show not found");
        };
        assert!(b);
    }
}
