use serde::Deserialize;

use actix_web::{dev::PeerAddr, get, http::Method, web, HttpRequest, HttpResponse};
use tracing::instrument;

mod ws;

use self::ws::websocket;
use crate::{
    errors::{GuardianError, Result},
    web::{helper, services::CATALOG},
};

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
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req.uri().path();

    let service = "notebook";

    // look up service and get url
    let catalog = helper::log_errors(CATALOG.get().ok_or_else(|| {
        GuardianError::GeneralError("Could not get catalog of services".to_string())
    }))?;
    let mut new_url = helper::log_errors(catalog.get(service))?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url).await
}

pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notebook")
            .service(notebook_ws_subscribe)
            .service(notebook_ws_session)
            .default_service(web::to(notebook_forward)),
    );
}
