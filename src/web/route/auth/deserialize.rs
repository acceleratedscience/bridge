use std::collections::HashMap;

use serde::de::Visitor;
use serde::Deserialize;

#[derive(Debug)]
pub struct TokenRequest {
    pub username: String,
    pub admin: String,
    pub gui: Option<bool>,
}

struct TokenRequestVisitor;

impl<'de> Visitor<'de> for TokenRequestVisitor {
    type Value = TokenRequest;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("TokenRequest")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let pairs: HashMap<&str, &str> = v
            .split('&')
            .filter_map(|s| {
                let mut parts = s.split('=');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    Some((key.trim(), value.trim_matches('"')))
                } else {
                    None
                }
            })
            .collect();

        let username = pairs
            .get("username")
            .ok_or_else(|| E::custom("Missing username"))?;
        let admin = pairs
            .get("admin")
            .ok_or_else(|| E::custom("Missing admin"))?;
        let gui = match pairs.get("gui") {
            Some(v) => Some(v.parse().map_err(E::custom)?),
            None => None,
        };

        Ok(TokenRequest {
            username: username.to_string(),
            admin: admin.to_string(),
            gui,
        })
    }
}

impl<'de> Deserialize<'de> for TokenRequest {
    fn deserialize<D>(deserializer: D) -> Result<TokenRequest, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "TokenRequest",
            &["username", "admin", "gui"],
            TokenRequestVisitor,
        )
    }
}

#[derive(Debug)]
pub struct CallBackResponse {
    pub code: String,
    pub state: String,
}

struct CallBackVisitor;

impl<'de> Visitor<'de> for CallBackVisitor {
    type Value = CallBackResponse;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("CallBackResponse")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let pairs: HashMap<&str, &str> = v
            .split('&')
            .filter_map(|s| {
                let mut parts = s.split('=');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    Some((key.trim(), value.trim_matches('"')))
                } else {
                    None
                }
            })
            .collect();

        let code = pairs.get("code").ok_or_else(|| E::custom("Missing code"))?;
        let state = pairs
            .get("state")
            .ok_or_else(|| E::custom("Missing state"))?;

        Ok(CallBackResponse {
            code: code.to_string(),
            state: state.to_string(),
        })
    }
}

impl<'de> Deserialize<'de> for CallBackResponse {
    fn deserialize<D>(deserializer: D) -> Result<CallBackResponse, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "CallBackResponse",
            &["code", "state"],
            CallBackVisitor,
        )
    }
}
