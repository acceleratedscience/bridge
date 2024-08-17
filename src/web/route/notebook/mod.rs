//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use serde::Deserialize;

use actix_web::{dev::PeerAddr, get, http::Method, web, HttpRequest, HttpResponse};
use tracing::instrument;

use crate::{
    errors::{GuardianError, Result},
    web::{helper, services::CATALOG},
};

#[get("/api/events/subscribe")]
async fn notebook_ws_subscribe(req: HttpRequest, pl: web::Payload) -> Result<HttpResponse> {
    helper::ws::manage_connection(req, pl, "ws://localhost:8888/notebook/api/events/subscribe")
        .await
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

    helper::ws::manage_connection(req, pl, url).await
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
    let catalog = helper::log_with_level!(
        CATALOG.get().ok_or_else(|| {
            GuardianError::GeneralError("Could not get catalog of services".to_string())
        }),
        error
    )?;
    let mut new_url = helper::log_with_level!(catalog.get(service), error)?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url).await
}

#[allow(dead_code)]
pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notebook")
            .service(notebook_ws_subscribe)
            .service(notebook_ws_session)
            .default_service(web::to(notebook_forward)),
    );
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_notebook_forward() {
        use actix_web::{http::header, test, web, App};
        use reqwest::Client;
        use serde_json::json;

        use crate::web::services::CATALOG;

        let mut catalog = CATALOG.get().unwrap();
        catalog.insert(
            "notebook".to_string(),
            "http://localhost:8888/notebook".parse().unwrap(),
        );

        let client = Client::new();

        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(client))
                .configure(super::config_notebook),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/notebook/api/sessions")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let body = String::from_utf8(body.to_vec()).unwrap();
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body, json!({}));
    }
}
