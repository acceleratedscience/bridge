//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use std::{marker::PhantomData, str::FromStr};

use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use mongodb::bson::doc;
use serde::Deserialize;

use actix_web::{
    cookie::Cookie,
    delete,
    dev::PeerAddr,
    get,
    http::Method,
    post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use tracing::instrument;
use url::Url;

use crate::{
    db::{
        models::{GuardianCookie, NotebookCookie, User, USER},
        mongo::{ObjectID, DB},
        Database,
    },
    errors::{GuardianError, Result},
    kube::{KubeAPI, Notebook, NotebookSpec, PVCSpec},
    web::{
        guardian_middleware::{CookieCheck, NotebookCookieCheck},
        helper::{self, bson},
    },
};

const NOTEBOOK_SUB_NAME: &str = "notebook";
const NOTEBOOK_PORT: &str = "8888";

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
        "ws://localhost:8888/{}/api/kernels/{}/channels?session_id={}",
        NOTEBOOK_SUB_NAME, kernel_id, session_id
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
        let id = ObjectID::new(&guardian_cookie.subject);

        // check if the user can create a notebook
        let result: Result<User> = db
            .find(
                doc! {
                    "_id": id.clone().into_inner(),
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

        // User is allowed to create a notebook, but notebook does not exist... so create one
        // Create a PVC at 1Gi
        let pvc = PVCSpec::new(
            notebook_helper::make_notebook_volume_name(&guardian_cookie.subject),
            1,
        );
        helper::log_with_level!(KubeAPI::new(pvc.spec).create().await, error)?;
        // Create a notebook
        let name = notebook_helper::make_notebook_name(&guardian_cookie.subject);
        let notebook = Notebook::new(
            &name,
            NotebookSpec::new(
                name.clone(),
                "quay-notebook-secret".to_string(),
                None,
                None,
                // TODO: Move this somewhere else
                Some(vec![
                    "--ServerApp.token=''".to_string(),
                    "--ServerApp.password=''".to_string(),
                    "--ServerApp.base_url='notebook'".to_string(),
                    "--ServerApp.allow_origin='*'".to_string(),
                    "--ServerApp.trust_xheaders=True".to_string(),
                    "--ServerApp.notebook_dir='/opt/app-root/src'".to_string(),
                    "--ServerApp.quit_button=False".to_string(),
                ]),
                "notebook-volume".to_string(),
                "/opt/app-root/src".to_string(),
            ),
        );
        helper::log_with_level!(KubeAPI::new(notebook).create().await, error)?;

        let current_time = time::OffsetDateTime::now_utc();
        db.update(
            doc! {
                "_id": id.into_inner(),
            },
            doc! {
                "$set": doc! {
                    "notebook": bson(current_time)?,
                },
            },
            USER,
            PhantomData::<User>,
        )
        .await?;

        // Create notebook cookie
        let notebook_cookie = Cookie::build("notebook", &guardian_cookie.subject)
            .path("/notebook")
            .secure(true)
            .http_only(true)
            .finish();

        return Ok(HttpResponse::Ok().cookie(notebook_cookie).finish());
    }

    helper::log_with_level!(
        Err(GuardianError::UserNotFound(
            "subject not passed from middleware".to_string(),
        )),
        error
    )
}

#[delete("/delete")]
async fn notebook_delete(notebook_cookie: Option<ReqData<NotebookCookie>>) -> Result<HttpResponse> {
    // get the notebook cookie
    let notebook_cookie = match notebook_cookie {
        Some(cookie) => cookie.into_inner(),
        None => {
            return helper::log_with_level!(
                Err(GuardianError::NotebookAccessError(
                    "Notebook cookie not found".to_string(),
                )),
                error
            )
        }
    };

    let name = notebook_helper::make_notebook_name(&notebook_cookie.subject);
    let pvc_name = notebook_helper::make_notebook_volume_name(&notebook_cookie.subject);
    helper::log_with_level!(KubeAPI::<Notebook>::delete(&name).await, error)?;
    helper::log_with_level!(
        KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name).await,
        error
    )?;
    Ok(HttpResponse::Ok().finish())
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

    // look up service and get url
    let mut new_url = Url::from_str("http://localhost:8888")?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url).await
}

mod notebook_helper {
    pub(super) fn make_notebook_name(subject: &str) -> String {
        format!("{}-notebook", subject)
    }

    pub(super) fn make_notebook_volume_name(subject: &str) -> String {
        format!("{}-notebook-volume", subject)
    }
}

pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notebook")
            .service(
                web::scope("")
                    .wrap(CookieCheck)
                    .service(notebook_create)
                    .service(notebook_delete),
            )
            .service(
                web::scope("")
                    .wrap(NotebookCookieCheck)
                    .service(notebook_ws_subscribe)
                    .service(notebook_ws_session)
                    .default_service(web::to(notebook_forward)),
            ),
    );
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_notebook_forward() {
        // assert!(true);
    }
}
