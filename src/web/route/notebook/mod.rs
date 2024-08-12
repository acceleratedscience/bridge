use std::str::FromStr;

use serde::Deserialize;

use actix_web::{
    dev::PeerAddr, get, http::{
        header::{HeaderName, HeaderValue},
        Method, StatusCode,
    }, web, HttpRequest, HttpResponse
};

use futures_util::StreamExt as _;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{error, instrument, warn};

mod ws;

use crate::{errors::{GuardianError, Result}, web::{helper, services::CATALOG}};
use self::ws::websocket;

#[get("/api/events/subscribe")]
async fn notebook_ws_subscribe(req: HttpRequest, pl: web::Payload) -> Result<HttpResponse> {
    websocket::manage_connection(req, pl, "ws://localhost:8888/notebook/api/events/subscribe").await
}

#[derive(Deserialize)]
struct Info {
    session_id: String,
}

#[get("/api/kernels/{kernel_id}/channels")]
async fn notebook_ws_session(
    req: HttpRequest,
    pl: web::Payload,
    kernel: web::Path<String>,
    session_id: web::Query<Info>,
) -> Result<HttpResponse> {
    let kernel_id = kernel.into_inner();
    let session_id = session_id.session_id.clone();

    let url = format!(
        "ws://localhost:8888/notebook/api/kernels/{}/channels?session_id={}",
        kernel_id, session_id
    );

    websocket::manage_connection(req, pl, url).await
}

#[instrument(skip(payload))]
async fn notebook_forward(
    req: HttpRequest,
    mut payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req
        .uri()
        .path();

    let service = "notebook";

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
    // for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
    //     let name = header_name.to_string();
    //     let value = header_value.to_str().unwrap();
    //
    //     let name = HeaderName::from_str(&name).unwrap();
    //     let value = HeaderValue::from_str(value).unwrap();
    //
    //     client_resp.insert_header((name, value));
    // }

    Ok(client_resp.streaming(res.bytes_stream()))
}

pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notebook")
            .service(notebook_ws_subscribe)
            .service(notebook_ws_session)
            .default_service(web::to(notebook_forward)),
    );
}
