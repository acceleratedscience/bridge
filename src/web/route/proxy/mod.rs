use std::str::FromStr;

use actix_web::{
    dev::PeerAddr,
    http::{
        header::{HeaderName, HeaderValue},
        Method, StatusCode,
    },
    web, HttpRequest, HttpResponse,
};
use actix_web_httpauth::middleware::HttpAuthentication;
use futures_util::StreamExt as _;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{error, instrument, warn};

use crate::{
    db, errors::{GuardianError, Result}, web::{guardian_middleware::validator, helper}
};

use self::services::CATALOG;

pub mod services;

const GUARDIAN_PREFIX: &str = "/proxy";
pub static INFERENCE_HEADER: &str = "Inference-Service";

#[instrument(skip(payload))]
async fn forward(
    req: HttpRequest,
    mut payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req
        .uri()
        .path()
        .strip_prefix(GUARDIAN_PREFIX)
        .unwrap_or(req.uri().path());

    // get header for which infernce service to forward to
    let service = req
        .headers()
        .get(INFERENCE_HEADER)
        .ok_or_else(|| {
            warn!("Inference-Service header not found in request");
            GuardianError::InferenceServiceHeaderNotFound
        })?
        .to_str()
        .map_err(|e| {
            error!("{:?}", e);
            GuardianError::InferenceServiceHeaderNotFound
        })?;

    // look up service and get url
    let catalog = helper::log_errors(CATALOG.get().ok_or_else(|| {
        GuardianError::GeneralError("Could not get catalog of services".to_string())
    }))?;
    let mut new_url = helper::log_errors(catalog.get(service))?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    let (tx, rx) = mpsc::unbounded_channel();

    actix_web::rt::spawn(async move {
        while let Some(chunk) = payload.next().await {
            tx.send(chunk).unwrap();
        }
    });

    // sigh...
    let method = match method.as_str() {
        "OPTIONS" => reqwest::Method::OPTIONS,
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "TRACE" => reqwest::Method::TRACE,
        "CONNECT" => reqwest::Method::CONNECT,
        "PATCH" => reqwest::Method::PATCH,
        _ => {
            return Err(GuardianError::GeneralError(
                "Unsupported HTTP method".to_string(),
            ))
        }
    };

    let forwarded_req = client
        .request(method, new_url)
        .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

    // TODO: This forwarded implementation is incomplete as it only handles the unofficial
    // X-Forwarded-For header but not the official Forwarded one.
    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => forwarded_req.header("X-Forwarded-For", addr.ip().to_string()),
        None => forwarded_req,
    };

    let res = helper::log_errors(forwarded_req.send().await.map_err(|e| {
        error!("{:?}", e);
        GuardianError::GeneralError(e.to_string())
    }))?;

    let status = res.status().as_u16();
    let status = StatusCode::from_u16(status).unwrap();

    let mut client_resp = HttpResponse::build(status);
    // Remove `Connection` as per
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        let name = header_name.to_string();
        let value = header_value.to_str().unwrap();

        let name = HeaderName::from_str(&name).unwrap();
        let value = HeaderValue::from_str(value).unwrap();

        client_resp.insert_header((name, value));
    }

    Ok(client_resp.streaming(res.bytes_stream()))
}

pub fn config_proxy(cfg: &mut web::ServiceConfig) {
    let auth_validator = HttpAuthentication::bearer(validator);
    cfg.service(
        web::scope("/proxy")
            .wrap(auth_validator)
            .default_service(web::to(forward)),
    );
}
