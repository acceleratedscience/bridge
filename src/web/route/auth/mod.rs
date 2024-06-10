use std::collections::HashMap;

use actix_web::{
    get,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use serde::{de::Visitor, Deserialize};
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    errors::{GuardianError, Result as GResult},
    web::helper,
};

#[derive(Debug)]
struct TokenRequest {
    username: String,
    admin: String,
    gui: Option<bool>,
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
        let gui = pairs
            .get("gui")
            .map(|&v| v.parse::<bool>().unwrap_or(false));

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

#[get("/get_token")]
#[instrument(skip(data))]
async fn get_token(data: Data<Tera>, req: HttpRequest) -> GResult<HttpResponse> {
    let query = req.query_string();
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let q = match TokenRequest::deserialize(deserializer) {
        Ok(q) => q,
        Err(e) => {
            return helper::log_errors(Err(GuardianError::QueryDeserializeError(e.to_string())))
        }
    };

    if let Some(true) = q.gui {
        let mut ctx = Context::new();
        ctx.insert("token", "123456789");
        let rendered = helper::log_errors(data.render("token.html", &ctx))?;

        Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
    } else {
        Ok(HttpResponse::Ok().json("123456789"))
    }
}

pub fn config_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/auth").service(get_token));
}
