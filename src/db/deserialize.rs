use std::fmt;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

use super::models::GroupForm;

struct GroupFormVisitor;

impl<'de> Deserialize<'de> for GroupForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(GroupFormVisitor)
    }
}

impl<'de> Visitor<'de> for GroupFormVisitor {
    type Value = GroupForm;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a URL encoded query string")
    }

    fn visit_str<E>(self, value: &str) -> Result<GroupForm, E>
    where
        E: de::Error,
    {
        let mut name = None;
        let mut subscriptions = Vec::new();
        let mut last_updated_by = None;

        for param in value.split('&') {
            let mut parts = param.split('=');
            let key = parts
                .next()
                .ok_or_else(|| de::Error::custom("missing key"))?;
            let val = parts
                .next()
                .ok_or_else(|| de::Error::custom("missing value"))?;
            let decoded_val = urlencoding::decode(val).map_err(de::Error::custom)?;

            match key {
                "name" => name = Some(decoded_val.to_string()),
                "subscriptions" => subscriptions.push(decoded_val.to_string()),
                "last_updated_by" => last_updated_by = Some(decoded_val.to_string()),
                _ => {
                    return Err(de::Error::unknown_field(
                        key,
                        ["name", "subscriptions", "last_updated_by"].as_ref(),
                    ))
                }
            }
        }

        let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
        let last_updated_by =
            last_updated_by.ok_or_else(|| de::Error::missing_field("last_updated_by"))?;

        Ok(GroupForm {
            name,
            subscriptions,
            last_updated_by,
        })
    }
}
