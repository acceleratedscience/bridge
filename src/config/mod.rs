use std::{
    collections::HashMap,
    fs::{self, read_to_string},
    path::PathBuf,
    str::FromStr,
    sync::LazyLock,
};

use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation};
use p256::{NistP256, elliptic_curve::JwkEcKey, pkcs8::DecodePublicKey};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::auth::openid::OpenIDProvider;

/// Singleton configuration object
pub struct Configuration {
    /// JWT encoder
    pub encoder: EncodingKey,
    /// JWT decoder
    pub decoder: DecodingKey,
    /// JWT validation
    pub validation: Validation,
    /// JSON Web Key ID
    pub kid: String,
    /// JSON Web Key
    pub jwk: JWK,
    /// Argon2 configuration... currently default
    pub argon_config: argon2::Config<'static>,
    pub db: Database,
    pub cache: CacheDB,
    #[cfg(feature = "notebook")]
    pub notebooks: HashMap<String, Notebook>,
    #[cfg(feature = "notebook")]
    pub notebook_namespace: String,
    pub app_name: String,
    pub app_discription: String,
    pub company: String,
    pub oidc: HashMap<String, OIDC>,
    pub observability_cred: Option<(String, String)>,
    #[cfg(feature = "openwebui")]
    pub owui_namespace: String,
}

pub struct Database {
    pub url: String,
    pub name: String,
}

pub struct CacheDB {
    pub url: String,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Notebook {
    pub url: String,
    pub pull_policy: String,
    pub working_dir: Option<String>,
    pub volume_mnt_path: Option<String>,
    pub notebook_env: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
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

#[derive(Serialize, Debug)]
pub struct JWK {
    pub kty: &'static str,
    pub kid: String,
    pub key_ops: Vec<&'static str>,
    pub crv: &'static str,
    pub alg: &'static str,
    pub x: String,
    pub y: String,
}

impl JWK {
    fn new(key: JwkEcKey, kid: String) -> Self {
        let encode = key.to_encoded_point::<NistP256>().unwrap();
        let x = BASE64_URL_SAFE_NO_PAD.encode(encode.x().unwrap());
        let y = BASE64_URL_SAFE_NO_PAD.encode(encode.y().unwrap());

        Self {
            kty: "EC",
            kid,
            key_ops: vec!["verify"],
            crv: "P-256",
            alg: "ES256",
            x,
            y,
        }
    }
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

    // JWK
    let mut hasher = sha2::Sha256::new();
    hasher.update(&public_key);
    let kid = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());
    let key = p256::PublicKey::from_public_key_pem(&String::from_utf8_lossy(&public_key)).unwrap();
    let jwk = JWK::new(key.to_jwk(), kid.clone());

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_audience(&AUD);
    validation.leeway = 0;

    let (config_location_str, database_location_str) = if cfg!(debug_assertions) {
        (
            "config/configurations_sample.toml",
            "config/database_sample.toml",
        )
    } else {
        ("config/configurations.toml", "config/database.toml")
    };

    let conf_table: toml::Table =
        toml::from_str(&read_to_string(PathBuf::from_str(config_location_str).unwrap()).unwrap())
            .unwrap();

    let db_table: toml::Table =
        toml::from_str(&read_to_string(PathBuf::from_str(database_location_str).unwrap()).unwrap())
            .unwrap();

    let mongo_table = db_table["mongodb"].as_table().unwrap();
    let db = Database {
        url: if cfg!(debug_assertions) {
            mongo_table["url_local"].as_str().unwrap().to_string()
        } else {
            mongo_table["url"].as_str().unwrap().to_string()
        },
        name: mongo_table["name"].as_str().unwrap().to_string(),
    };

    let cache_db = db_table["keydb"].as_table().unwrap();
    let cache = CacheDB {
        url: if cfg!(debug_assertions) {
            cache_db["url_local"].as_str().unwrap().to_string()
        } else {
            cache_db["url"].as_str().unwrap().to_string()
        },
        name: cache_db["name"].as_str().unwrap().to_string(),
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

    let observability_cred = match (
        app_conf["observability_access_token"]
            .as_str()
            .map(|s| s.to_string()),
        app_conf["observability_endpoint"]
            .as_str()
            .map(|s| s.to_string()),
    ) {
        (Some(token), Some(endpoint)) => Some((token, endpoint)),
        _ => None,
    };

    #[cfg(feature = "notebook")]
    let notebook_namespace = app_conf["notebook_namespace"].as_str().unwrap().to_string();
    #[cfg(feature = "notebook")]
    let notebooks: HashMap<String, Notebook> = toml::from_str(
        &read_to_string(PathBuf::from_str("config/notebook.toml").unwrap()).unwrap(),
    )
    .unwrap();

    #[cfg(feature = "openwebui")]
    let owui_namespace = app_conf["owui_namespace"].as_str().unwrap().to_string();

    Configuration {
        encoder,
        decoder,
        validation,
        jwk,
        kid,
        argon_config: argon2::Config::default(),
        db,
        cache,
        #[cfg(feature = "notebook")]
        notebooks,
        #[cfg(feature = "notebook")]
        notebook_namespace,
        app_name,
        app_discription,
        company,
        oidc: oidc_map,
        observability_cred,
        #[cfg(feature = "openwebui")]
        owui_namespace,
    }
}

#[cfg(test)]
mod tests {

    use crate::auth::jwt::{self};

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

    #[test]
    fn test_jwk() {
        let config = init_once();

        let jwt = jwt::get_token_and_exp(
            &config.encoder,
            86400,
            "sub",
            AUD[0],
            vec!["role".to_string()],
        )
        .unwrap()
        .0;

        let jwk = serde_json::to_string(&config.jwk).unwrap();
        let mut jwk = jsonwebkey::JsonWebKey::from_str(&jwk).unwrap();
        assert!(jwk.set_algorithm(jsonwebkey::Algorithm::ES256).is_ok());

        let pk = jwk.key.to_pem();
        let decoder = DecodingKey::from_ec_pem(pk.as_bytes()).unwrap();

        assert!(jwt::validate_token(&jwt, &decoder, &config.validation).is_ok());
    }

    #[test]
    fn observability_refresh_token() {
        let config = init_once();
        assert!(config.observability_cred.is_some());
        let cred = config.observability_cred.as_ref().unwrap();
        assert!(!cred.1.is_empty());
        assert!(cred.1.len() > 10);
    }
}
