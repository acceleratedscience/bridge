use std::str::FromStr;

use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::StatusCode;
use actix_web::{
    dev::PeerAddr, error, web, Error, HttpRequest, HttpResponse,
};
use futures_util::StreamExt as _;
use reqwest::Method;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use url::Url;

const REQWEST_PREFIX: &str = "/using-reqwest";

async fn forward_reqwest(
    req: HttpRequest,
    mut payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    url: web::Data<Url>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse, Error> {
    let path = req
        .uri()
        .path()
        .strip_prefix(REQWEST_PREFIX)
        .unwrap_or(req.uri().path());

    let mut new_url = (**url).clone();
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    let (tx, rx) = mpsc::unbounded_channel();

    actix_web::rt::spawn(async move {
        while let Some(chunk) = payload.next().await {
            tx.send(chunk).unwrap();
        }
    });

    let forwarded_req = client
        .request(method, new_url)
        .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

    // TODO: This forwarded implementation is incomplete as it only handles the unofficial
    // X-Forwarded-For header but not the official Forwarded one.
    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => forwarded_req.header("x-forwarded-for", addr.ip().to_string()),
        None => forwarded_req,
    };

    let res = forwarded_req
        .send()
        .await
        .map_err(error::ErrorInternalServerError)?;

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
