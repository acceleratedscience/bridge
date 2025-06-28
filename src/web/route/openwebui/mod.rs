use actix_web::{
    HttpRequest, HttpResponse,
    dev::PeerAddr,
    get, guard,
    http::Method,
    web::{self, ReqData},
};
use tracing::instrument;

use crate::{
    config::CONFIG,
    db::models::BridgeCookie,
    errors::Result,
    web::{helper, notebook_helper},
};

const OWUI: &str = "owui";

#[get("ws/socket.io/")]
async fn openwebui_ws(
    req: HttpRequest,
    pl: web::Payload,
    bridge_cookie: Option<ReqData<BridgeCookie>>,
) -> Result<HttpResponse> {
    // ws://localhost:8000/ws/socket.io/?EIO=4&transport=websocket
    Ok(HttpResponse::SwitchingProtocols()
        .header("Sec-WebSocket-Protocol", "websocket")
        .finish())
}

#[instrument(skip(payload))]
async fn openwebui_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    bridge_cookie: Option<ReqData<BridgeCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    todo!()
}

pub fn config_openwebui(cfg: &mut web::ServiceConfig) {
    cfg.service(openwebui_ws)
        .default_service(web::to(openwebui_forward));
}
