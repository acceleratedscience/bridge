use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tracing::log::info;

use crate::errors::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    iss: String,
    exp: usize,
    sub: String,
}

const ISSUER: &str = "guardian";

/// Generate a token with the given lifetime and uuid. This is an expensive operation, cache as
/// much as possible
pub fn get_token(key: &EncodingKey, token_lifetime: usize, sub: String) -> Result<String> {
    // Get exp UNIX EPOC
    let start = SystemTime::now();
    let since_epoc = start.duration_since(UNIX_EPOCH)?;
    let exp = since_epoc.as_secs() as usize;

    let claims = Claims {
        iss: ISSUER.to_owned(),
        exp: exp + token_lifetime,
        sub,
    };
    let token = encode(&Header::new(Algorithm::ES256), &claims, key)?;
    Ok(token)
}

/// Validate token. If token is valid, return the claims, is not return an error
pub fn validate_token(token: &str, decode_key: &DecodingKey, val: &Validation) -> Result<Claims> {
    let token = decode::<Claims>(token, decode_key, val)?;
    Ok(token.claims)
}
