use std::fmt;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

use super::models::{GroupForm, UserDeleteForm, UserForm, UserType};

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

struct UserFormVisitor;

impl<'de> Deserialize<'de> for UserForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UserFormVisitor)
    }
}

impl<'de> Visitor<'de> for UserFormVisitor {
    type Value = UserForm;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a URL encoded query string")
    }

    fn visit_str<E>(self, value: &str) -> Result<UserForm, E>
    where
        E: de::Error,
    {
        let mut email = None;
        let mut groups = Vec::new();
        let mut user_type = None;
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
                "email" => email = Some(decoded_val.to_string()),
                "groups" => groups.push(decoded_val.to_string()),
                "user_type" => user_type = Some(decoded_val.to_string()),
                "last_updated_by" => last_updated_by = Some(decoded_val.to_string()),
                _ => {
                    return Err(de::Error::unknown_field(
                        key,
                        ["email", "groups", "user_type", "last_updated_by"].as_ref(),
                    ))
                }
            }
        }

        let email = email.ok_or_else(|| de::Error::missing_field("name"))?;
        let user_type: Option<UserType> = user_type.map(|v| v.as_str().into());
        let last_updated_by =
            last_updated_by.ok_or_else(|| de::Error::missing_field("last_updated_by"))?;

        Ok(UserForm {
            email,
            groups,
            user_type,
            last_updated_by,
        })
    }
}

struct UserDeleteFormVisitor;

impl<'de> Deserialize<'de> for UserDeleteForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UserDeleteFormVisitor)
    }
}

impl<'de> Visitor<'de> for UserDeleteFormVisitor {
    type Value = UserDeleteForm;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a URL encoded query string")
    }

    fn visit_str<E>(self, value: &str) -> Result<UserDeleteForm, E>
    where
        E: de::Error,
    {
        let mut email = None;
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
                "email" => email = Some(decoded_val.to_string()),
                "last_updated_by" => last_updated_by = Some(decoded_val.to_string()),
                _ => {
                    return Err(de::Error::unknown_field(
                        key,
                        ["email", "last_updated_by"].as_ref(),
                    ))
                }
            }
        }

        let email = email.ok_or_else(|| de::Error::missing_field("name"))?;
        let last_updated_by =
            last_updated_by.ok_or_else(|| de::Error::missing_field("last_updated_by"))?;

        Ok(UserDeleteForm {
            email,
            last_updated_by,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::db::models::UserForm;

    use super::*;

    #[test]
    fn test_group_form_deserialize() {
        let form = "name=group1&subscriptions=sub1&last_updated_by=me";
        let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(form);
        let result = GroupForm::deserialize(deserializer).unwrap();
        assert_eq!(result.name, "group1");
        assert!(result.subscriptions.contains(&"sub1".to_string()));
        assert_eq!(result.last_updated_by, "me");
    }

    #[test]
    fn test_user_form_deserialize() {
        let form = "sub=123&groups=group1&user_type=system&last_updated_by=me";
        let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(form);
        let result = UserForm::deserialize(deserializer).unwrap();
        assert_eq!(result.email, "123");
        assert_eq!(result.groups, vec!["group1"]);
        assert_eq!(result.user_type, Some(UserType::SystemAdmin));
        assert_eq!(result.last_updated_by, "me");
    }
}
