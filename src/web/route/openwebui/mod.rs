use actix_web::{
    HttpRequest, HttpResponse,
    dev::PeerAddr,
    http::Method,
    web::{self, ReqData},
};
use tracing::instrument;

use crate::{
    db::models::BridgeCookie,
    errors::Result,
    web::{helper, notebook_helper},
};

const OWUI: &str = "owui";

async fn openwebui_ws() {}

#[instrument(skip(payload))]
async fn openwebui_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    bridge_cookie: Option<ReqData<BridgeCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req.uri().path();
    //  TODO: check for some auth here

    let mut new_url = Url::from_str(&notebook_helper::make_forward_url(
        &bridge_cookie.ip,
        &notebook_helper::make_notebook_name(&bridge_cookie.subject),
        "http",
        None,
    ))?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url, None).await
}
