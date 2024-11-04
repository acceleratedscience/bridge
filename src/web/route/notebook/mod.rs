//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use std::str::FromStr;

use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;

use actix_web::{
    delete,
    dev::PeerAddr,
    get,
    http::Method,
    post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use tracing::instrument;

use crate::{
    config::CONFIG, db::{
        models::{GuardianCookie, User, USER},
        mongo::DB,
        Database,
    }, errors::{GuardianError, Result}, kube::{KubeAPI, Notebook, NotebookSpec}, web::{helper, services::CATALOG}
};

const NOTEBOOK_SUB_NAME: &str = "notebook";

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

#[post("/create")]
async fn notebook_create(
    subject: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    if let Some(_subject) = subject {
        let guardian_cookie = _subject.into_inner();
        let id = ObjectId::from_str(&guardian_cookie.subject)
            .map_err(|e| GuardianError::GeneralError(e.to_string()))?;

        // check if the user can create a notebook
        let result: Result<User> = db
            .find(
                doc! {
                    "_id": id,
                },
                USER,
            )
            .await;
        let user = match result {
            Ok(user) => {
                if !user.sub.contains(NOTEBOOK_SUB_NAME) {
                    return Err(GuardianError::NotebookAccessError(
                        "User not allowed to create a notebook".to_string(),
                    ));
                }
                user
            }
            Err(e) => return helper::log_with_level!(Err(e), error),
        };
        if user.notebook.is_some() {
            return Err(GuardianError::NotebookExistsError(
                "Notebook already exists".to_string(),
            ));
        };

        // Create a notebook
        let name = format!("notebook-{}", user._id);
        let notebook = Notebook::new(
            &name,
            NotebookSpec::new(
                name.clone(),
                CONFIG.notebook_image.clone(),
                "IfNotPresent".to_string(),
                "quay-notebook-secret".to_string(),
            ),
        );
        KubeAPI::new(notebook).create().await?;

        return Ok(HttpResponse::Ok().finish());
    }

    helper::log_with_level!(
        Err(GuardianError::UserNotFound(
            "subject not passed from middleware".to_string(),
        )),
        error
    )
}

#[delete("/delete")]
async fn notebook_delete() -> HttpResponse {
    HttpResponse::Ok().finish()
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
    let mut new_url = helper::log_with_level!(CATALOG.get(service), error)?;
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
        // assert!(true);
    }
}
