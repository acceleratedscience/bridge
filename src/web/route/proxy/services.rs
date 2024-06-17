use std::str::FromStr;
use std::{fs::read_to_string, path::PathBuf, sync::OnceLock};

use url::Url;

use crate::errors::{GuardianError, Result};

pub struct Catalog(pub toml::Table);

pub static CATALOG: OnceLock<Catalog> = OnceLock::new();

pub fn init_once() {
    let table = toml::from_str(
        &read_to_string(PathBuf::from_str("config/services.toml").unwrap()).unwrap(),
    )
    .unwrap();
    CATALOG.get_or_init(|| Catalog(table));
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

    pub fn list(&self) -> Vec<Url> {
        self.0.get("services");
        todo!();
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
}
