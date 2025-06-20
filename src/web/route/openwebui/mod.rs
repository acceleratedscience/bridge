use actix_web::{
    HttpRequest, HttpResponse,
    dev::PeerAddr,
    http::Method,
    web::{self, ReqData},
};
use tracing::instrument;

use crate::{
    errors::Result,
    web::{helper, notebook_helper},
};

async fn openwebui_ws() {}

#[instrument(skip(payload))]
async fn openwebui_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    // notebook_cookie: Option<ReqData<NotebookCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req.uri().path();
    /* Example
        Path: /notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js
        New URL: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook
        New URL with path: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js
        New URL with query: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js?v=79b385776e13e3f97005
    */

    // check for some auth here

    let mut new_url = Url::from_str(&notebook_helper::make_forward_url(
        &notebook_cookie.ip,
        &notebook_helper::make_notebook_name(&notebook_cookie.subject),
        "http",
        None,
    ))?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url, None).await
}
