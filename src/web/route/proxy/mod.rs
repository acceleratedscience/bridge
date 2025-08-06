use actix_web::{HttpRequest, HttpResponse, dev::PeerAddr, http::Method, web};
use actix_web_httpauth::middleware::HttpAuthentication;
use tracing::{error, instrument, warn};

use crate::{
    errors::{BridgeError, Result},
    web::{
        bridge_middleware::validator,
        helper::{self, forwarding::Config},
    },
};

use self::services::CATALOG;

pub mod services;

const BRIDGE_PREFIX: &str = "/proxy";
pub static INFERENCE_HEADER: &str = "Inference-Service";

#[instrument(skip(payload))]
async fn forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req
        .uri()
        .path()
        .strip_prefix(BRIDGE_PREFIX)
        .unwrap_or(req.uri().path());

    // get header for which infernce service to forward to
    let service = req
        .headers()
        .get(INFERENCE_HEADER)
        .ok_or_else(|| {
            warn!("Inference-Service header not found in request");
            BridgeError::InferenceServiceHeaderNotFound
        })?
        .to_str()
        .map_err(|e| {
            error!("{:?}", e);
            BridgeError::InferenceServiceHeaderNotFound
        })?;

    // look up service and get url
    let mut new_url = helper::log_with_level!(CATALOG.get_service(service), error)?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(
        req,
        payload,
        method,
        peer_addr,
        client,
        new_url,
        Config {
            inference: true,
            ..Default::default()
        },
    )
    .await
}

pub fn config_proxy(cfg: &mut web::ServiceConfig) {
    let auth_validator = HttpAuthentication::bearer(validator);
    cfg.service(
        web::scope(BRIDGE_PREFIX)
            .wrap(auth_validator)
            .default_service(web::to(forward)),
    );
}
