use std::fs;
use std::sync::OnceLock;

use jsonwebtoken::{DecodingKey, Validation, Algorithm, EncodingKey};
use tracing::warn;

pub struct Configuration {
    pub encoder: EncodingKey,
    pub decoder: DecodingKey,
    pub validation: Validation,
}

pub static CONFIG: OnceLock<Configuration> = OnceLock::new();

pub fn init_once() {
    let private_key = fs::read("certs/private.ec.key").unwrap();
    let private_key = crate::auth::sec1_to_pkcs8(&private_key);
    let encoder = EncodingKey::from_ec_pem(&private_key).unwrap();

    let public_key = fs::read("certs/public-key.pem").unwrap();
    let decoder = DecodingKey::from_ec_pem(&public_key).unwrap();

    let mut validation = Validation::new(Algorithm::ES256);
    validation.leeway = 0;

    if CONFIG
        .set(Configuration {
            encoder,
            decoder,
            validation,
        })
        .is_err()
    {
        warn!("Configuration already set.");
    }
}
