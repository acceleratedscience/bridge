use std::{
    collections::HashMap,
    fs::{self, read_to_string},
    path::PathBuf,
    str::FromStr,
    sync::LazyLock,
};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation};
use serde::Deserialize;

use crate::auth::openid::OpenIDProvider;

pub struct Configuration {
    pub encoder: EncodingKey,
    pub decoder: DecodingKey,
    pub validation: Validation,
    pub argon_config: argon2::Config<'static>,
    pub db: Database,
    pub cache: CacheDB,
    pub notebooks: HashMap<String, Notebook>,
    pub app_name: String,
    pub app_discription: String,
    pub company: String,
    pub oidc: HashMap<String, OIDC>,
}

pub struct Database {
    pub url: String,
}

pub struct CacheDB {
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct Notebook {
    pub url: String,
    pub pull_policy: String,
    pub working_dir: Option<String>,
    pub volume_mnt_path: Option<String>,
    pub env: Option<Vec<String>>,
    pub secret: Option<String>,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub start_up_url: Option<String>,
    pub max_idle_time: Option<u64>,
}

const OIDC_PROVIDER: [OpenIDProvider; 2] = [OpenIDProvider::W3, OpenIDProvider::IbmId];
pub struct OIDC {
    pub client: String,
    pub url: String,
    pub redirect_url: String,
    pub client_id: String,
    pub client_secret: String,
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

    let conf_table: toml::Table = toml::from_str(
        &read_to_string(PathBuf::from_str("config/configurations.toml").unwrap()).unwrap(),
    )
    .unwrap();

    let db_table: toml::Table = toml::from_str(
        &read_to_string(PathBuf::from_str("config/database.toml").unwrap()).unwrap(),
    )
    .unwrap();

    let mongo_table = db_table["mongodb"].as_table().unwrap();
    let db = Database {
        url: if cfg!(debug_assertions) {
            mongo_table["url_local"].as_str().unwrap().to_string()
        } else {
            mongo_table["url"].as_str().unwrap().to_string()
        },
    };

    let cache_db = db_table["keydb"].as_table().unwrap();
    let cache = CacheDB {
        url: if cfg!(debug_assertions) {
            cache_db["url_local"].as_str().unwrap().to_string()
        } else {
            cache_db["url"].as_str().unwrap().to_string()
        },
    };

    let mut oidc_map: HashMap<String, OIDC> = HashMap::with_capacity(2);
    OIDC_PROVIDER.into_iter().for_each(|provider| {
        let provider: &str = provider.into();
        let openid_table = conf_table[provider].as_table().unwrap();
        let client = openid_table["client"].as_table().unwrap();

        let url = openid_table["url"].as_str().unwrap().to_string();
        let redirect_url = openid_table["redirect_url"].as_str().unwrap().to_string();

        let client_id = client["client_id"].as_str().unwrap().to_string();
        let client_secret = client["client_secret"].as_str().unwrap().to_string();
        let oidc = OIDC {
            client: provider.into(),
            url,
            redirect_url,
            client_id,
            client_secret,
        };

        oidc_map.insert(provider.into(), oidc);
    });

    let app_conf = conf_table["app-config"].as_table().unwrap();
    let app_name = app_conf["name"].as_str().unwrap().to_string();
    let app_discription = app_conf["description"].as_str().unwrap().to_string();
    let company = app_conf["company"].as_str().unwrap().to_string();

    let notebooks: HashMap<String, Notebook> = toml::from_str(
        &read_to_string(PathBuf::from_str("config/notebook.toml").unwrap()).unwrap(),
    )
    .unwrap();

    Configuration {
        encoder,
        decoder,
        validation,
        argon_config: argon2::Config::default(),
        db,
        cache,
        notebooks,
        app_name,
        app_discription,
        company,
        oidc: oidc_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_init_once() {
        let config = init_once();
        let workbench = config.notebooks.get("open_ad_workbench").unwrap();
        assert_eq!(workbench.pull_policy, "Always");
        assert_eq!(workbench.working_dir, Some("/opt/app-root/src".to_string()));
        assert_eq!(
            workbench.start_up_url,
            Some("lab/tree/start_menu.ipynb".to_string())
        );
    }
}
