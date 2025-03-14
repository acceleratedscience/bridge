use std::collections::HashMap;

use serde::{Deserialize, de::Visitor};

#[allow(dead_code)]
#[derive(Debug)]
pub struct CallBackResponse {
    pub code: String,
    pub state: String,
}

struct CallBackVisitor;

impl Visitor<'_> for CallBackVisitor {
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
        deserializer.deserialize_struct("CallBackResponse", &["code", "state"], CallBackVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize() {
        let query = "code=123&state=456";
        let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
        let callback_response = CallBackResponse::deserialize(deserializer).unwrap();

        assert_eq!(callback_response.code, "123");
        assert_eq!(callback_response.state, "456");
    }
}
