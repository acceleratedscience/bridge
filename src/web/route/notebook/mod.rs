//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use std::{marker::PhantomData, str::FromStr};

use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};
use mongodb::bson::doc;
use serde::Deserialize;

use actix_web::{
    cookie::{self, Cookie, SameSite},
    delete,
    dev::PeerAddr,
    get,
    http::{header::ContentType, Method, StatusCode},
    post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use tera::{Context, Tera};
use tracing::{info, instrument, warn};
use url::Url;

use crate::{
    auth::{NOTEBOOK_COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME},
    db::{
        models::{
            Group, GuardianCookie, NotebookCookie, NotebookStatusCookie, User, UserNotebook, GROUP,
            USER,
        },
        mongo::{ObjectID, DB},
        Database,
    },
    errors::{GuardianError, Result},
    kube::{KubeAPI, Notebook, NotebookSpec, PVCSpec, NAMESPACE},
    web::{
        guardian_middleware::{CookieCheck, Htmx, NotebookCookieCheck},
        helper::{self, bson},
    },
};

pub const NOTEBOOK_SUB_NAME: &str = NAMESPACE;
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
    data: Data<Tera>,
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
                    "updated_at": bson(time::OffsetDateTime::now_utc())?,
                    "notebook": bson(current_time)?,
                },
            },
            USER,
            PhantomData::<User>,
        )
        .await?;

        let notebook_cookie = NotebookCookie {
            subject: guardian_cookie.subject,
        };
        let notebook_status_cookie = NotebookStatusCookie {
            start_time: current_time.to_string(),
            status: "Pending".to_string(),
        };
        let notebook_json = serde_json::to_string(&notebook_cookie).map_err(|e| {
            GuardianError::GeneralError(format!("Could not serialize notebook cookie: {}", e))
        })?;
        let notebook_status_json = serde_json::to_string(&notebook_status_cookie).map_err(|e| {
            GuardianError::GeneralError(format!(
                "Could not serialize notebook status cookie: {}",
                e
            ))
        })?;

        // Create notebook cookies
        let notebook_cookie = Cookie::build(NOTEBOOK_COOKIE_NAME, &notebook_json)
            .path("/notebook")
            .same_site(SameSite::Strict)
            .secure(true)
            .http_only(true)
            .max_age(time::Duration::days(1))
            .finish();
        let notebook_status_cookie =
            Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, &notebook_status_json)
                .path("/")
                .same_site(SameSite::Strict)
                .secure(true)
                .http_only(true)
                .max_age(time::Duration::days(1))
                .finish();

        let content = helper::log_with_level!(
            data.render("components/notebook/poll.html", &Context::new()),
            error
        )?;

        return Ok(HttpResponse::Ok()
            .cookie(notebook_cookie)
            .cookie(notebook_status_cookie)
            .content_type(ContentType::form_url_encoded())
            .body(content));
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
    data: Data<Tera>,
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
            "$set": doc! {
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
                "notebook": null,
            },
        },
        USER,
        PhantomData::<User>,
    )
    .await?;

    // delete the cookies
    let mut notebook_cookie = Cookie::build(NOTEBOOK_COOKIE_NAME, "")
        .path("/notebook")
        .same_site(SameSite::Strict)
        .secure(true)
        .http_only(true)
        .finish();
    let mut notebook_status_cookie = Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, "")
        .path("/")
        .same_site(SameSite::Strict)
        .secure(true)
        .http_only(true)
        .finish();
    notebook_cookie.make_removal();
    notebook_status_cookie.make_removal();

    let content = helper::log_with_level!(
        data.render("components/notebook/start.html", &Context::new()),
        error
    )?;

    Ok(HttpResponse::Ok()
        .cookie(notebook_cookie)
        .cookie(notebook_status_cookie)
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[get("status")]
async fn notebook_status(
    data: Data<Tera>,
    subject: Option<ReqData<GuardianCookie>>,
    notebook_cookie: Option<ReqData<NotebookStatusCookie>>,
) -> Result<HttpResponse> {
    let (guardian_cookie, notebook_cookie) = match (subject, notebook_cookie) {
        (Some(gc), Some(ncs)) => (gc.into_inner(), ncs.into_inner()),
        _ => {
            return helper::log_with_level!(
                Err(GuardianError::NotebookAccessError(
                    "Guardian cookie and or notebook_status_cookie not found".to_string(),
                )),
                error
            )
        }
    };

    // check status on k8s
    let ready = KubeAPI::<Pod>::check_pod_running(
        &(notebook_helper::make_notebook_name(&guardian_cookie.subject) + "-0"),
    )
    .await?;

    if ready {
        let notebook_status_cookie = NotebookStatusCookie {
            start_time: notebook_cookie.start_time,
            status: "Ready".to_string(),
        };
        let notebook_status_json = serde_json::to_string(&notebook_status_cookie).map_err(|e| {
            GuardianError::GeneralError(format!(
                "Could not serialize notebook status cookie: {}",
                e
            ))
        })?;
        let notebook_status_cookie_updated =
            Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, &notebook_status_json)
                .path("/")
                .same_site(SameSite::Strict)
                .secure(true)
                .http_only(true)
                .max_age(time::Duration::days(1))
                .finish();

        let notebook = Into::<UserNotebook>::into((&guardian_cookie, &notebook_status_cookie));
        let mut ctx = Context::new();
        ctx.insert("notebook", &notebook);
        let content =
            helper::log_with_level!(data.render("components/notebook/ready.html", &ctx), error)?;

        // Status code 286 is HTMX specific to stop caller from polling again
        return Ok(HttpResponse::build(StatusCode::from_u16(286).unwrap())
            .content_type(ContentType::form_url_encoded())
            .cookie(notebook_status_cookie_updated)
            .body(content));
    }

    // This will make the htmx on client side poll
    Ok(HttpResponse::ServiceUnavailable().finish())
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

pub mod notebook_helper {
    use crate::db::models::{GuardianCookie, NotebookStatusCookie, User, UserNotebook};
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
            // TODO: This is super cumbersome... FIX IT FIX IT!
            Some(p) => format!(
                "{}://{}-0.{}.pod.cluster.local:{}/notebook/{}/{}/{}",
                protocol, name, NAMESPACE, NOTEBOOK_PORT, NAMESPACE, name, p
            ),
            None => format!(
                "{}://{}-0.{}.pod.cluster.local:{}/notebook/{}/{}",
                protocol, name, NAMESPACE, NOTEBOOK_PORT, NAMESPACE, name
            ),
        }
    }

    pub(super) fn make_path(name: &str, path: Option<&str>) -> String {
        match path {
            Some(p) => format!("/notebook/{}/{}/{}", NAMESPACE, name, p),
            None => format!("/notebook/{}/{}", NAMESPACE, name),
        }
    }

    impl From<&User> for UserNotebook {
        fn from(user: &User) -> Self {
            UserNotebook {
                url: make_path(&make_notebook_name(&user._id.to_string()), None),
                name: user._id.to_string(),
                start_time: user
                    .notebook
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "None".to_string()),
                status: "Pending".to_string(),
            }
        }
    }

    impl From<(&GuardianCookie, &NotebookStatusCookie)> for UserNotebook {
        fn from(
            (guardian_cookie, notebook_status_cookie): (&GuardianCookie, &NotebookStatusCookie),
        ) -> Self {
            UserNotebook {
                url: make_path(&make_notebook_name(&guardian_cookie.subject), None),
                name: guardian_cookie.subject.clone(),
                start_time: notebook_status_cookie.start_time.clone(),
                status: notebook_status_cookie.status.clone(),
            }
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
        web::scope("/notebook_manage/hx")
            .wrap(CookieCheck)
            // .wrap(Htmx)
            .service(notebook_create)
            .service(notebook_delete)
            .service(notebook_status),
    );
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_notebook_forward() {
        // assert!(true);
    }
}
