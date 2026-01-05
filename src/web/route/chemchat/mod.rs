use std::str::FromStr;

use actix_web::{
    HttpRequest, HttpResponse,
    dev::PeerAddr,
    http::Method,
    web::{self, ReqData},
};
use tracing::instrument;
use url::Url;

use crate::{
    config::CONFIG,
    db::models::BridgeCookie,
    errors::{BridgeError, Result},
    web::{
        bridge_middleware::ResourceCookieCheck,
        helper::{self, forwarding},
    },
};

use super::resource::resource_http;

// todo: move this to config
const CHEMCHAT_NAME: &str = "main-api";

#[instrument(skip(payload))]
async fn chemchat_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    bridge_cookie: Option<ReqData<BridgeCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    bridge_cookie
        .as_ref()
        .and_then(|bc| bc.resources.as_ref())
        .filter(|resources| resources.iter().any(|r| r == CHEMCHAT_NAME))
        .ok_or_else(|| BridgeError::Unauthorized("Access denied to chemchat".to_string()))?;

    let mut url = Url::from_str(&CONFIG.chemchat_internal_url)?;
    let path = req.path();
    url.set_path(path);
    url.set_query(req.uri().query());

    helper::forwarding::forward(
        req,
        payload,
        method,
        peer_addr,
        client,
        url,
        forwarding::Config {
            ..Default::default()
        },
    )
    .await
}

pub fn config_chemchat(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/resource")
            .wrap(ResourceCookieCheck)
            .default_service(web::to(resource_http)),
    );
    cfg.service(web::scope("").default_service(web::to(chemchat_forward)));
}
