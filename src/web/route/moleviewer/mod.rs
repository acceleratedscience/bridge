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
    web::helper::{self, forwarding},
};

#[instrument(skip(payload))]
async fn moleviewer_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    bridge_cookie: Option<ReqData<BridgeCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    if bridge_cookie.is_none() {
        return Err(BridgeError::Unauthorized(
            "Bridge cookie not found when trying to access moleviewer".to_string(),
        ));
    }

    let mut url = Url::from_str(&CONFIG.moleviewer_internal_url)?;
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

pub fn config_moleviewer(cfg: &mut web::ServiceConfig) {
    cfg.default_service(web::to(moleviewer_forward));
}
