use std::{str::FromStr, time::Duration};

use actix_web::{HttpRequest, HttpResponse, dev::PeerAddr, web};
use bytes::Bytes;
use futures::StreamExt;
use http::{
    Method, Request, Uri,
    header::{HeaderName, HeaderValue},
};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;
use url::Url;

use crate::errors::{BridgeError, Result};

use super::ws::{forward_ws, is_websocket};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type BrideStream = StreamBody<ReceiverStream<std::result::Result<Frame<Bytes>, BoxError>>>;

pub fn bridge_payload(mut pl: web::Payload) -> BrideStream {
    let (tx, rx) = mpsc::channel::<std::result::Result<Frame<Bytes>, BoxError>>(64);

    actix_web::rt::spawn(async move {
        while let Some(chunk) = pl.next().await {
            let frame = chunk.map(Frame::data).map_err(|e| Box::new(e) as BoxError);
            if tx.send(frame).await.is_err() {
                // upstream closed — stop reading
                break;
            }
        }
    });

    StreamBody::new(ReceiverStream::new(rx))
}

// Was going to use this for GET AND HEAD request... but decide to not enforce no body on these
// request if the client sends one. So we just forward the body if it exists.
// use http_body_util::{Empty, combinators::BoxBody};
// pub fn empty_body() -> BoxBody<Bytes, BoxError> {
//     Empty::new().map_err(|never| match never {}).boxed()
// }

const HOP_BY_HOP_REQ: &[&str] = &["connection", "keep-alive", "host", "content-length"];
const HOP_BY_HOP_RESP: &[&str] = &[
    "connection",
    "keep-alive",
    "content-length",
    "transfer-encoding",
];

pub async fn forward(
    req: HttpRequest,
    payload: web::Payload,
    method: actix_web::http::Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<crate::web::proxy_client::ProxyClient>,
    new_url: Url,
    config: crate::web::helper::Config<'_>,
) -> Result<HttpResponse> {
    // WS check
    if is_websocket(&req) {
        return forward_ws(req, payload, peer_addr, new_url).await;
    }

    let method = Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let inference = config.inference;
    let pack_cookies = config.pack_cookies;
    let updated_cookie = config.updated_cookie;

    let uri =
        Uri::from_str(new_url.as_str()).map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let mut builder = Request::builder().method(method).uri(uri);
    let headers = builder.headers_mut().ok_or_else(|| {
        BridgeError::GeneralError("Failed to get headers from request builder".to_string())
    })?;

    for (name, value) in req.headers() {
        if inference && matches!(name.as_str(), "authorization" | "inference-service") {
            continue;
        }
        // skip hop-by-hop headers on the way in, too
        if HOP_BY_HOP_REQ.contains(&name.as_str()) {
            continue;
        }

        let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            warn!(header = name.as_str(), "dropping unconvertible header");
            continue; // drop, don't panic
        };

        headers.append(n, v); // append, not insert — preserves duplicate headers
    }

    if let Some(PeerAddr(addr)) = peer_addr {
        // append to existing chain instead of clobbering it
        let prior = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok());
        let xff = match prior {
            Some(p) => format!("{p}, {}", addr.ip()),
            None => addr.ip().to_string(),
        };
        if let Ok(v) = HeaderValue::from_str(&xff) {
            headers.insert("x-forwarded-for", v);
        }
    }
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    // find all the cookie header and pack them delimited by ";"
    if pack_cookies {
        let mut cookies = String::new();
        for (header_name, header_value) in req.headers().iter() {
            if header_name.as_str().to_lowercase() == "cookie"
                && let Ok(value) = header_value.to_str()
            {
                if !cookies.is_empty() {
                    cookies.push(';');
                }
                cookies.push_str(value);
            }
        }
        if !cookies.is_empty() {
            headers.insert(
                HeaderName::from_static("cookie"),
                HeaderValue::from_str(&cookies).unwrap(),
            );
        }
    }

    let stream = http_body_util::BodyExt::boxed(bridge_payload(payload));
    let req = builder
        .body(stream)
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let res = tokio::time::timeout(Duration::from_hours(1), client.into_inner().0.request(req))
        .await
        .map_err(|_| BridgeError::GeneralError("Request timed out".to_string()))?
        .map_err(|e| {
            BridgeError::GeneralError(format!("Failed to send request to target: {}", e))
        })?;

    let status = actix_web::http::StatusCode::from_u16(res.status().as_u16())
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let mut client_resp = HttpResponse::build(status);
    for (name, value) in res.headers() {
        if HOP_BY_HOP_RESP.contains(&name.as_str()) {
            continue;
        }

        match (
            actix_web::http::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            actix_web::http::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            (Ok(n), Ok(v)) => {
                client_resp.append_header((n, v));
            }
            _ => continue,
        }
    }

    if let Some(cookie) = updated_cookie {
        client_resp.cookie(cookie);
    };

    let body_stream = res
        .into_body()
        .into_data_stream()
        .map(|r| r.map_err(|e| BridgeError::GeneralError(e.to_string())));

    Ok(client_resp.streaming(body_stream))
}

#[cfg(test)]
mod test {}
