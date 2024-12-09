use std::{
    collections::HashMap,
    fs::{self, read_to_string},
    path::PathBuf,
    str::FromStr,
    sync::LazyLock,
};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation};
use serde::Deserialize;

pub struct Configuration {
    pub encoder: EncodingKey,
    pub decoder: DecodingKey,
    pub validation: Validation,
    pub db: Database,
    pub notebooks: HashMap<String, Notebook>,
}

pub struct Database {
    pub url: String,
}

#[derive(Deserialize)]
pub struct Notebook {
    pub url: String,
    pub pull_policy: String,
    pub working_dir: Option<String>,
    pub volume_mnt_path: Option<String>,
    pub env: Option<Vec<String>>,
    pub secret: Option<String>,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
}

pub static CONFIG: LazyLock<Configuration> = LazyLock::new(init_once);

pub static AUD: [&str; 1] = ["openad-user"];

pub fn init_once() -> Configuration {
    // TODO: remove these unwraps
    // For now, it's ok because we run this once at start up and we should not start application
    // without these files
    let private_key = fs::read("certs/private.ec.key").unwrap();
    let private_key = crate::auth::sec1_to_pkcs8(&private_key);
    let encoder = EncodingKey::from_ec_pem(&private_key).unwrap();

    let public_key = fs::read("certs/public-key.pem").unwrap();
    let decoder = DecodingKey::from_ec_pem(&public_key).unwrap();

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_audience(&AUD);
    validation.leeway = 0;

    let db_table: toml::Table = toml::from_str(
        &read_to_string(PathBuf::from_str("config/database.toml").unwrap()).unwrap(),
    )
    .unwrap();

    let db_table = db_table["database"].as_table().unwrap();

    let db = Database {
        url: if cfg!(debug_assertions) {
            db_table["url_local"].as_str().unwrap().to_string()
        } else {
            db_table["url"].as_str().unwrap().to_string()
        },
    };

    let notebooks: HashMap<String, Notebook> = toml::from_str(
        &read_to_string(PathBuf::from_str("config/notebook.toml").unwrap()).unwrap(),
    )
    .unwrap();

    Configuration {
        encoder,
        decoder,
        validation,
        db,
        notebooks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_init_once() {
        let config = init_once();
        let workbench = config.notebooks.get("open_ad_workbench").unwrap();
        assert_eq!(workbench.pull_policy, "IfNotPresent");
        assert_eq!(workbench.working_dir, Some("/opt/app-root/src".to_string()));
    }
}
