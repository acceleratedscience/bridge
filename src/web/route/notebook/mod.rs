use actix_web::{
    get,
    web::{self},
    HttpRequest, HttpResponse,
};
use serde::Deserialize;

mod ws;

use crate::errors::Result;
use self::ws::websocket;

#[get("/api/events/subscribe")]
async fn notebook_websocket(req: HttpRequest, pl: web::Payload) -> Result<HttpResponse> {
    websocket::manage_connection(req, pl, "ws://localhost:8888/proxy/api/events/subscribe").await
}

#[derive(Deserialize)]
struct Info {
    session_id: String,
}

#[get("/api/kernels/{kernel_id}/channels")]
pub async fn notebook_session(
    req: HttpRequest,
    pl: web::Payload,
    kernel: web::Path<String>,
    session_id: web::Query<Info>,
) -> Result<HttpResponse> {
    let kernel_id = kernel.into_inner();
    let session_id = session_id.session_id.clone();

    let url = format!(
        "ws://localhost:8888/proxy/api/kernels/{}/channels?session_id={}",
        kernel_id, session_id
    );

    websocket::manage_connection(req, pl, url).await
}
