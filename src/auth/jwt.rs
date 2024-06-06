// use std::time::{SystemTime, UNIX_EPOCH};
//
// use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
// use openssl::{ec, pkey::PKey};
// use serde::{Deserialize, Serialize};
// use tracing::log::info;
// use uuid::Uuid;
//
// #[derive(Debug, Serialize, Deserialize)]
// pub struct Claims {
//     iss: String,
//     exp: usize,
//     sub: String,
// }
//
// const ISSUER: &str = "guardian";
//
// pub fn get_token(key: &EncodingKey, token_lifetime: usize, uuid: Uuid) -> Result<String> {
//     // Get exp UNIX EPOC
//     let start = SystemTime::now();
//     let since_epoc = start.duration_since(UNIX_EPOCH)?;
//     info!("{}", since_epoc.as_secs());
//     let exp = since_epoc.as_secs() as usize;
//     info!("{}", exp + token_lifetime);
//
//     let claims = Claims {
//         iss: ISSUER.to_owned(),
//         exp: exp + token_lifetime,
//         sub: uuid.to_string(),
//     };
//     let token = encode(&Header::new(Algorithm::ES256), &claims, key)?;
//     Ok(token)
// }
//
// pub fn validate_token(token: &str, decode_key: &DecodingKey, val: &Validation) -> Result<Claims> {
//     let token = decode::<Claims>(token, decode_key, val)?;
//     Ok(token.claims)
// }
//
// pub fn sec1_to_pkcs8(secret_pem: &[u8]) -> Vec<u8> {
//     // Convert SEC1 to PKCS8
//     // See note here: https://github.com/Keats/jsonwebtoken#convert-sec1-private-key-to-pkcs8
//     let key = ec::EcKey::private_key_from_pem(secret_pem).unwrap();
//     let key = PKey::from_ec_key(key).unwrap();
//     key.private_key_to_pem_pkcs8().unwrap()
// }
