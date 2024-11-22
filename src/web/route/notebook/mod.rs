//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use std::{marker::PhantomData, str::FromStr};

use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use mongodb::bson::doc;
use serde::Deserialize;

use actix_web::{
    cookie::{Cookie, SameSite},
    delete,
    dev::PeerAddr,
    get,
    http::Method,
    post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use tracing::{info, instrument, warn};
use url::Url;

use crate::{
    db::{
        models::{Group, GuardianCookie, NotebookCookie, User, GROUP, USER},
        mongo::{ObjectID, DB},
        Database,
    },
    errors::{GuardianError, Result},
    kube::{KubeAPI, Notebook, NotebookSpec, PVCSpec, NAMESPACE},
    web::{
        guardian_middleware::{CookieCheck, NotebookCookieCheck},
        helper::{self, bson},
    },
};

const NOTEBOOK_SUB_NAME: &str = NAMESPACE;
const NOTEBOOK_PORT: &str = "8888";

#[get("{name}/api/events/subscribe")]
async fn notebook_ws_subscribe(
    req: HttpRequest,
    pl: web::Payload,
    notebook_cookie: Option<ReqData<NotebookCookie>>,
) -> Result<HttpResponse> {
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
    let url = notebook_helper::make_forward_url(
        &notebook_helper::make_notebook_name(&notebook_cookie.subject),
        "ws",
        Some("api/events/subscribe"),
    );

    helper::ws::manage_connection(req, pl, url).await
}

#[derive(Deserialize)]
struct Info {
    session_id: String,
}

#[get("{name}/api/kernels/{kernel_id}/channels")]
async fn notebook_ws_session(
    req: HttpRequest,
    pl: web::Payload,
    kernel: web::Path<(String, String)>,
    session_id: web::Query<Info>,
    notebook_cookie: Option<ReqData<NotebookCookie>>,
) -> Result<HttpResponse> {
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

    let kernel_id = kernel.into_inner().1;
    let session_id = session_id.session_id.clone();

    let path = format!(
        "api/kernels/{}/channels?session_id={}",
        kernel_id, session_id
    );
    let url = notebook_helper::make_forward_url(
        &notebook_helper::make_notebook_name(&notebook_cookie.subject),
        "ws",
        Some(&path),
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
        let user: User = helper::log_with_level!(
            db.find(
                doc! {
                    "_id": id.clone().into_inner(),
                },
                USER,
            )
            .await,
            error
        )?;
        let group: Group = helper::log_with_level!(
            db.find(
                doc! {
                    "name": &user.groups[0]
                },
                GROUP,
            )
            .await,
            error
        )?;

        if !group.subscriptions.contains(&NOTEBOOK_SUB_NAME.to_string()) {
            warn!(
                "User {} does not have permission to create a notebook",
                guardian_cookie.subject
            );
            return Err(GuardianError::NotebookAccessError(
                "User does not have permission to create a notebook".to_string(),
            ));
        }

        if user.notebook.is_some() {
            warn!(
                "Notebook already exists for user {}",
                guardian_cookie.subject
            );
            return Err(GuardianError::NotebookExistsError(
                "Notebook already exists".to_string(),
            ));
        };

        // Create notebook namespace if it does not exist
        // TODO: Maybe move this to only do this once when the application starts up...
        if helper::log_with_level!(
            KubeAPI::<Notebook>::make_namespace(NOTEBOOK_SUB_NAME).await,
            error
        )?
        .is_some()
        {
            info!("Namespace {} has been created", NAMESPACE)
        }

        // User is allowed to create a notebook, but notebook does not exist... so create one
        // Create a PVC at 1Gi
        let pvc_name = notebook_helper::make_notebook_volume_name(&guardian_cookie.subject);
        let pvc = PVCSpec::new(pvc_name.clone(), 1);
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
                    format!("--ServerApp.base_url='notebook/{}/{}'", NAMESPACE, name),
                    "--ServerApp.notebook_dir='/opt/app-root/src'".to_string(),
                    "--ServerApp.quit_button=False".to_string(),
                ]),
                pvc_name,
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
        let notebook_cookie = Cookie::build(NOTEBOOK_SUB_NAME, &guardian_cookie.subject)
            .path("/notebook")
            .same_site(SameSite::Strict)
            .secure(true)
            .http_only(true)
            .max_age(time::Duration::days(1))
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
async fn notebook_delete(
    subject: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the notebook cookie
    let guardian_cookie = match subject {
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

    let name = notebook_helper::make_notebook_name(&guardian_cookie.subject);
    let pvc_name = notebook_helper::make_notebook_volume_name(&guardian_cookie.subject);
    helper::log_with_level!(KubeAPI::<Notebook>::delete(&name).await, error)?;
    helper::log_with_level!(
        KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name).await,
        error
    )?;

    db.update(
        doc! {
            "_id": ObjectID::new(&guardian_cookie.subject).into_inner(),
        },
        doc! {
            "$unset": doc! {
                "notebook": "",
            },
        },
        USER,
        PhantomData::<User>,
    )
    .await?;

    // delete the cookie
    let mut notebook_cookie = Cookie::build(NOTEBOOK_SUB_NAME, "")
        .path("/notebook")
        .same_site(SameSite::Strict)
        .secure(true)
        .http_only(true)
        .finish();

    notebook_cookie.make_removal();

    Ok(HttpResponse::Ok().cookie(notebook_cookie).finish())
}

#[instrument(skip(payload))]
async fn notebook_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    notebook_cookie: Option<ReqData<NotebookCookie>>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let path = req.uri().path();

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

    // look up service and get url
    let mut new_url = Url::from_str(&notebook_helper::make_forward_url(
        &notebook_helper::make_notebook_name(&notebook_cookie.subject),
        "http",
        None,
    ))?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, new_url).await
}

mod notebook_helper {
    use crate::kube::NAMESPACE;
    use crate::web::route::notebook::NOTEBOOK_PORT;

    pub(super) fn make_notebook_name(subject: &str) -> String {
        format!("{}-notebook", subject)
    }

    pub(super) fn make_notebook_volume_name(subject: &str) -> String {
        format!("{}-notebook-volume-pvc", subject)
    }

    pub(super) fn make_forward_url(name: &str, protocol: &str, path: Option<&str>) -> String {
        if cfg!(debug_assertions) {
            return match path {
                Some(p) => format!(
                    "{}://localhost:{}/notebook/{}/{}/{}",
                    protocol, NOTEBOOK_PORT, NAMESPACE, name, p
                ),
                None => format!(
                    "{}://localhost:{}/notebook/{}/{}",
                    protocol, NOTEBOOK_PORT, NAMESPACE, name
                ),
            };
        }
        match path {
            Some(p) => format!(
                "{}://{}.{}.svc.cluster.local:{}/notebook/{}/{}/{}",
                protocol, name, NAMESPACE, NOTEBOOK_PORT, NAMESPACE, name, p
            ),
            None => format!(
                "{}://{}.{}.svc.cluster.local:{}/notebook/{}/{}",
                protocol, name, NAMESPACE, NOTEBOOK_PORT, NAMESPACE, name
            ),
        }
    }
}

pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope(&("/notebook/".to_string() + NAMESPACE))
            .wrap(NotebookCookieCheck)
            .service(notebook_ws_subscribe)
            .service(notebook_ws_session)
            .default_service(web::to(notebook_forward)),
    )
    .service(
        web::scope("/notebook_manage")
            .wrap(CookieCheck)
            .service(notebook_create)
            .service(notebook_delete),
    );
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_notebook_forward() {
        // assert!(true);
    }
}
