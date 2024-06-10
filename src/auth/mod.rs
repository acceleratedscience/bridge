use openssl::{ec, pkey::PKey};

pub mod jwt;

/// Convert SEC1 to PKCS8
pub fn sec1_to_pkcs8(secret_pem: &[u8]) -> Vec<u8> {
    // See note here: https://github.com/Keats/jsonwebtoken#convert-sec1-private-key-to-pkcs8
    let key = ec::EcKey::private_key_from_pem(secret_pem).unwrap();
    let key = PKey::from_ec_key(key).unwrap();
    key.private_key_to_pem_pkcs8().unwrap()
}
