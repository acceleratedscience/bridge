use std::{
    fs::{self, read_to_string},
    path::PathBuf,
    str::FromStr,
    sync::LazyLock,
};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation};

pub struct Configuration {
    pub encoder: EncodingKey,
    pub decoder: DecodingKey,
    pub validation: Validation,
    pub db: Database,
}

pub struct Database {
    pub url: String,
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

    let table: toml::Table = toml::from_str(
        &read_to_string(PathBuf::from_str("config/database.toml").unwrap()).unwrap(),
    )
    .unwrap();

    let db_table = table["database"].as_table().unwrap();

    let db = Database {
        url: if cfg!(debug_assertions) {
            db_table["url_local"].as_str().unwrap().to_string()
        } else {
            db_table["url"].as_str().unwrap().to_string()
        },
    };

    Configuration {
        encoder,
        decoder,
        validation,
        db,
    }
}
