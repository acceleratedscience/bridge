use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::errors::Result;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims<T>
where
    T: AsRef<str>,
{
    iss: String,
    exp: usize,
    sub: T,
    aud: T,
    pub scp: Vec<String>,
}

impl<T> Claims<T>
where
    T: AsRef<str>,
{
    pub fn token_exp_as_string(&self) -> String {
        match time::OffsetDateTime::from_unix_timestamp(self.exp as i64) {
            Ok(d) => d.to_string(),
            Err(_) => "Not a valid timestamp".to_string(),
        }
    }

    pub fn get_sub(&self) -> &str {
        self.sub.as_ref()
    }
}

const ISSUER: &str = "bridge";

/// Generate a token with the given lifetime and uuid. This is an expensive operation, cache as
/// much as possible
pub fn get_token_and_exp(
    key: &EncodingKey,
    token_lifetime: usize,
    sub: &str,
    aud: &str,
    scp: Vec<String>,
) -> Result<(String, String)> {
    // Get exp UNIX EPOC
    let start = SystemTime::now();
    let since_epoc = start.duration_since(UNIX_EPOCH)?;
    let exp = since_epoc.as_secs() as usize;

    let claims = Claims {
        iss: ISSUER.to_owned(),
        exp: exp + token_lifetime,
        sub,
        aud,
        scp,
    };
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(CONFIG.kid.clone());

    let token = encode(&header, &claims, key)?;
    Ok((token, claims.token_exp_as_string()))
}

/// Validate token. If token is valid, return the claims, is not return an error
pub fn validate_token(
    token: &str,
    decode_key: &DecodingKey,
    val: &Validation,
) -> Result<Claims<String>> {
    let token = decode::<Claims<String>>(token, decode_key, val)?;
    Ok(token.claims)
}
