//! This module contains the proxy logic for the Juptyer Notebook. In order to proxy traffic to
//! notebook, we use the forward function from the helper module. But we also introduce to
//! websocket endpoints.

use std::{marker::PhantomData, str::FromStr, time::Duration};

use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};
use mongodb::bson::doc;
use serde::Deserialize;

use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite},
    delete,
    dev::PeerAddr,
    get,
    http::{Method, StatusCode, header::ContentType},
    post,
    web::{self, Data, ReqData},
};
use tera::{Context, Tera};
use tracing::{info, instrument, warn};
use url::Url;

use crate::{
    auth::{COOKIE_NAME, NOTEBOOK_COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME, jwt},
    config::{AUD, CONFIG},
    db::{
        Database,
        models::{
            BridgeCookie, Config, GROUP, Group, NotebookCookie, NotebookInfo, NotebookStatusCookie,
            USER, User, UserNotebook,
        },
        mongo::{DB, ObjectID},
    },
    errors::{BridgeError, Result},
    kube::{KubeAPI, NAMESPACE, Notebook, NotebookSpec, PVCSpec},
    web::{
        bridge_middleware::{CookieCheck, Htmx, NotebookCookieCheck},
        helper::{self, bson},
    },
};

pub const NOTEBOOK_SUB_NAME: &str = "notebook";
const NOTEBOOK_PORT: &str = "8888";
const NOTEBOOK_TOKEN_LIFETIME: usize = const { 60 * 60 * 24 * 30 };
const PVC_DELETE_ATTEMPT: u8 = 9;

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
                Err(BridgeError::NotebookAccessError(
                    "Notebook cookie not found".to_string(),
                )),
                error
            );
        }
    };
    let url = notebook_helper::make_forward_url(
        &notebook_cookie.ip,
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
                Err(BridgeError::NotebookAccessError(
                    "Notebook cookie not found".to_string(),
                )),
                error
            );
        }
    };

    let kernel_id = kernel.into_inner().1;
    let session_id = session_id.session_id.clone();

    let path = format!(
        "api/kernels/{}/channels?session_id={}",
        kernel_id, session_id
    );
    let url = notebook_helper::make_forward_url(
        &notebook_cookie.ip,
        &notebook_helper::make_notebook_name(&notebook_cookie.subject),
        "ws",
        Some(&path),
    );

    helper::ws::manage_connection(req, pl, url).await
}

#[post("/create")]
async fn notebook_create(
    req: HttpRequest,
    subject: Option<ReqData<BridgeCookie>>,
    // payload: web::Payload,
    db: Data<&DB>,
    data: Data<Tera>,
) -> Result<HttpResponse> {
    if let Some(_subject) = subject {
        let mut bridge_cookie = _subject.into_inner();
        let id = ObjectID::new(&bridge_cookie.subject);

        // Defaulting to true with ui update... this is to simiply the pvc option. Instead of
        // giving the user the option to persist the pvc before start a workbench, we ask when the
        // user want to terminate workbench whether they want to persist the pvc or not.
        // To ensure we can continue to give this option despite the notebook lifecycle, we set
        // this to true by default.
        let persist_pvc = true;
        // let pl = payload.to_bytes().await?;
        // if String::from_utf8_lossy(&pl).eq("volume=on") {
        //     persist_pvc = true;
        // }
        bridge_cookie.config = Some(Config {
            notebook_persist_pvc: Some(persist_pvc),
        });

        // check if the user can create a notebook
        // TODO: instead of two separate calls to the DB, do this in one call
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
                bridge_cookie.subject
            );
            return Err(BridgeError::NotebookAccessError(
                "User does not have permission to create a notebook".to_string(),
            ));
        }

        let scp = if user.groups.is_empty() {
            vec!["".to_string()]
        } else {
            group.subscriptions
        };

        let (notebook_token, _) = jwt::get_token_and_exp(
            &CONFIG.encoder,
            NOTEBOOK_TOKEN_LIFETIME,
            &bridge_cookie.subject,
            AUD[0],
            scp,
        )?;

        if user.notebook.is_some() {
            warn!("Notebook already exists for user {}", bridge_cookie.subject);
            return Err(BridgeError::NotebookExistsError(
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
            info!("Namespace {} has been created", *NAMESPACE)
        }

        // User is allowed to create a notebook, but notebook does not exist... so create one
        // Create a PVC at 1Gi
        let pvc_name = notebook_helper::make_notebook_volume_name(&bridge_cookie.subject);

        if req.query_string().contains("clear") {
            helper::log_with_level!(
                KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name).await,
                error
            )?;

            // PVC takes time to delete... loop and check it is gone
            loop {
                let mut loop_cnt = 0;
                if KubeAPI::<PersistentVolumeClaim>::check_pvc_exists(&pvc_name).await? {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    loop_cnt += 1;
                } else {
                    break;
                }

                if loop_cnt > PVC_DELETE_ATTEMPT {
                    let apx_time_elapsed = PVC_DELETE_ATTEMPT * loop_cnt;
                    warn!(
                        "PVC {} not deleted after {} seconds",
                        pvc_name, apx_time_elapsed
                    );
                    return Err(BridgeError::GeneralError(
                        "PVC not deleted after extended period of time".to_string(),
                    ));
                }
            }
        }

        let pvc = PVCSpec::new(pvc_name.clone(), 1);
        if let Err(e) = KubeAPI::new(pvc.spec).create().await {
            match e {
                BridgeError::NotebookExistsError(_) => info!(
                    "PVC {} already exists, most likely persisted from last session, reusing",
                    pvc_name
                ),
                _ => helper::log_with_level!(Err(e), error)?,
            }
        }
        // Create a notebook
        let name = notebook_helper::make_notebook_name(&bridge_cookie.subject);
        let mut start_up_url = None;
        let mut max_idle_time = None;
        let notebook = Notebook::new(
            &name,
            NotebookSpec::new(
                name.clone(),
                "open_ad_workbench",
                pvc_name,
                &mut start_up_url,
                &mut max_idle_time,
                vec![("PROXY_KEY".to_string(), notebook_token)],
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
                    "updated_at": bson(current_time)?,
                    "notebook": bson(NotebookInfo{
                        start_time: Some(current_time),
                        last_active: None,
                        max_idle_time,
                        start_up_url: start_up_url.clone(),
                        persist_pvc,
                    })?,
                    "last_updated_by": &user.sub,
                },
            },
            USER,
            PhantomData::<User>,
        )
        .await?;

        let bridge_cookie_json = serde_json::to_string(&bridge_cookie).map_err(|e| {
            BridgeError::GeneralError(format!("Could not serialize bridge cookie: {}", e))
        })?;
        let notebook_cookie = NotebookCookie {
            subject: bridge_cookie.subject,
            ip: String::new(),
        };
        let notebook_status_cookie = NotebookStatusCookie {
            start_time: current_time.to_string(),
            status: "Pending".to_string(),
            start_url: start_up_url,
        };
        let notebook_json = serde_json::to_string(&notebook_cookie).map_err(|e| {
            BridgeError::GeneralError(format!("Could not serialize notebook cookie: {}", e))
        })?;
        let notebook_status_json = serde_json::to_string(&notebook_status_cookie).map_err(|e| {
            BridgeError::GeneralError(format!("Could not serialize notebook status cookie: {}", e))
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
        let bridge_cookie = Cookie::build(COOKIE_NAME, bridge_cookie_json)
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
            .cookie(bridge_cookie)
            .content_type(ContentType::form_url_encoded())
            .body(content));
    }

    helper::log_with_level!(
        Err(BridgeError::UserNotFound(
            "subject not passed from middleware".to_string(),
        )),
        error
    )
}

#[delete("/delete")]
async fn notebook_delete(
    req: HttpRequest,
    subject: Option<ReqData<BridgeCookie>>,
    data: Data<Tera>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the notebook cookie
    let bridge_cookie = match subject {
        Some(cookie) => cookie.into_inner(),
        None => {
            return helper::log_with_level!(
                Err(BridgeError::NotebookAccessError(
                    "Notebook cookie not found".to_string(),
                )),
                error
            );
        }
    };

    // get user data
    // TODO: we should swap bridge_cookie.subject with email, and not do this db call
    let user: User = helper::log_with_level!(
        db.find(
            doc! {
                "_id": ObjectID::new(&bridge_cookie.subject).into_inner(),
            },
            USER,
        )
        .await,
        error
    )?;
    let persist_pvc = req.query_string().contains("save");

    helper::utils::notebook_destroy(**db, &bridge_cookie.subject, persist_pvc, &user.email).await?;

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

    let mut ctx = Context::new();
    ctx.insert("cooloff", &true);
    #[cfg(feature = "notebook")]
    {
        if let Some(ref conf) = bridge_cookie.config {
            // TODO:: check if it's ok to pass in Option instead of bool
            ctx.insert("pvc", &conf.notebook_persist_pvc);
        }
        if persist_pvc {
            ctx.insert("pvc_exists", &true);
        }
    }

    println!("Context: {:?}", ctx);

    let content =
        helper::log_with_level!(data.render("components/notebook/start.html", &ctx), error)?;

    Ok(HttpResponse::Ok()
        .cookie(notebook_cookie)
        .cookie(notebook_status_cookie)
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[get("status")]
async fn notebook_status(
    data: Data<Tera>,
    client: Data<reqwest::Client>,
    subject: Option<ReqData<BridgeCookie>>,
    nsc: Option<ReqData<NotebookStatusCookie>>,
) -> Result<HttpResponse> {
    let (bridge_cookie, nsc) = match (subject, nsc) {
        (Some(gc), Some(ncs)) => (gc.into_inner(), ncs.into_inner()),
        _ => {
            return helper::log_with_level!(
                Err(BridgeError::NotebookAccessError(
                    "Bridge cookie and or notebook_status_cookie not found".to_string(),
                )),
                error
            );
        }
    };

    // check status on k8s
    let ready = KubeAPI::<Pod>::check_pod_running(
        &(notebook_helper::make_notebook_name(&bridge_cookie.subject) + "-0"),
    )
    .await?;

    if ready {
        let ip = KubeAPI::<Pod>::get_pod_ip(
            &(notebook_helper::make_notebook_name(&bridge_cookie.subject) + "-0"),
        )
        .await?;

        // ping the notebook for 200 status, that way we only serve notebook when it is ready
        let url = notebook_helper::make_forward_url(
            &ip,
            &notebook_helper::make_notebook_name(&bridge_cookie.subject),
            "http",
            None,
        );
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| BridgeError::GeneralError(e.to_string()))?;
        if !resp.status().is_success() {
            return Ok(HttpResponse::ServiceUnavailable().finish());
        }

        // essentially updating the cookies
        let notebook_status_cookie = NotebookStatusCookie {
            start_time: nsc.start_time,
            status: "Ready".to_string(),
            start_url: nsc.start_url,
        };
        let notebook_cookie = NotebookCookie {
            subject: bridge_cookie.subject.clone(),
            ip,
        };

        let notebook_status_json =
            serde_json::to_string(&notebook_status_cookie).map_err(|er| {
                BridgeError::GeneralError(format!(
                    "Could not serialize notebook status cookie: {}",
                    er
                ))
            })?;
        let notebook_cookie_json = serde_json::to_string(&notebook_cookie).map_err(|er| {
            BridgeError::GeneralError(format!("Could not serialize notebook cookie: {}", er))
        })?;

        // TODO: We are leveraing cookies to avoid DB calls... but look into not doing this anymore
        let notebook_status_cookie_updated =
            Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, &notebook_status_json)
                .path("/")
                .same_site(SameSite::Strict)
                .secure(true)
                .http_only(true)
                .max_age(time::Duration::days(1))
                .finish();
        let notebook_cookie_updated = Cookie::build(NOTEBOOK_COOKIE_NAME, &notebook_cookie_json)
            .path("/notebook")
            .same_site(SameSite::Strict)
            .secure(true)
            .http_only(true)
            .max_age(time::Duration::days(1))
            .finish();

        let notebook = Into::<UserNotebook>::into((bridge_cookie, notebook_status_cookie));
        let mut ctx = Context::new();
        ctx.insert("notebook", &notebook);
        let content =
            helper::log_with_level!(data.render("components/notebook/ready.html", &ctx), error)?;

        // Status code 286 is HTMX specific to stop caller from polling again
        return Ok(HttpResponse::build(StatusCode::from_u16(286).unwrap())
            .content_type(ContentType::form_url_encoded())
            .cookie(notebook_status_cookie_updated)
            .cookie(notebook_cookie_updated)
            .body(content));
    }

    // This will make the htmx on client side poll
    Ok(HttpResponse::ServiceUnavailable().finish())
}

#[post("enter")]
async fn notebook_enter(
    bridge_cookie: Option<ReqData<BridgeCookie>>,
    nsc: Option<ReqData<NotebookStatusCookie>>,
    db: Data<&DB>,
    data: Data<Tera>,
) -> Result<HttpResponse> {
    let nsc = nsc
        .ok_or(BridgeError::NotebookAccessError(
            "Notebook status cookie not found".to_string(),
        ))?
        .into_inner();
    let bridge_cookie = bridge_cookie
        .ok_or(BridgeError::NotebookAccessError(
            "Bridge cookie not found".to_string(),
        ))?
        .into_inner();

    let new_nsc = NotebookStatusCookie {
        start_time: nsc.start_time.clone(),
        status: nsc.status.clone(),
        start_url: None,
    };

    let user: User = db
        .find(
            doc! {
                "_id": ObjectID::new(&bridge_cookie.subject).into_inner(),
            },
            USER,
        )
        .await?;

    let now = time::OffsetDateTime::now_utc();
    // update last_active to now
    db.update(
        doc! {
            "_id": ObjectID::new(&bridge_cookie.subject).into_inner(),
        },
        doc! {
            "$set": doc! {
                "notebook.last_active": bson(now)?,
                "updated_at": bson(now)?,
                "last_updated_by": &user.sub,
            },
        },
        USER,
        PhantomData::<User>,
    )
    .await?;

    // update notebook status cookie
    let notebook_status_json =
        serde_json::to_string(&new_nsc).map_err(|e| BridgeError::GeneralError(e.to_string()))?;
    let notebook_status_cookie = Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, &notebook_status_json)
        .path("/")
        .same_site(SameSite::Strict)
        .secure(true)
        .http_only(true)
        .max_age(time::Duration::days(1))
        .finish();

    let user_notebook = Into::<UserNotebook>::into((bridge_cookie, new_nsc));
    let mut ctx = Context::new();
    ctx.insert("notebook", &user_notebook);

    let content = data
        .render("components/notebook/ready.html", &ctx)
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .cookie(notebook_status_cookie)
        .body(content))
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
    /* Example
        Path: /notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js
        New URL: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook
        New URL with path: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js
        New URL with query: http://localhost:8888/notebook/notebook/675fe4d56881c0dbd5cc2960-notebook/static/lab/main.79b385776e13e3f97005.js?v=79b385776e13e3f97005
    */

    let notebook_cookie = match notebook_cookie {
        Some(cookie) => cookie.into_inner(),
        None => {
            return helper::log_with_level!(
                Err(BridgeError::NotebookAccessError(
                    "Notebook cookie not found".to_string(),
                )),
                error
            );
        }
    };

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

pub mod notebook_helper {
    use crate::{
        db::models::{BridgeCookie, NotebookInfo, NotebookStatusCookie, User, UserNotebook},
        kube::NAMESPACE,
        web::route::notebook::NOTEBOOK_PORT,
    };

    pub(crate) fn make_notebook_name(subject: &str) -> String {
        format!("{}-notebook", subject)
    }

    pub(crate) fn make_notebook_volume_name(subject: &str) -> String {
        format!("{}-notebook-volume-pvc", subject)
    }

    pub(crate) fn make_forward_url(
        ip: &str,
        name: &str,
        protocol: &str,
        path: Option<&str>,
    ) -> String {
        if cfg!(debug_assertions) {
            return match path {
                Some(p) => format!(
                    "{}://localhost:{}/notebook/{}/{}/{}",
                    protocol, NOTEBOOK_PORT, *NAMESPACE, name, p
                ),
                None => format!(
                    "{}://localhost:{}/notebook/{}/{}",
                    protocol, NOTEBOOK_PORT, *NAMESPACE, name
                ),
            };
        }
        match path {
            // TODO: This is super cumbersome... FIX IT FIX IT!
            Some(p) => format!(
                "{}://{}:{}/notebook/{}/{}/{}",
                protocol, ip, NOTEBOOK_PORT, *NAMESPACE, name, p
            ),
            None => format!(
                "{}://{}:{}/notebook/{}/{}",
                protocol, ip, NOTEBOOK_PORT, *NAMESPACE, name
            ),
        }
    }

    pub(super) fn make_path(name: &str, path: Option<&str>) -> String {
        match path {
            Some(p) => format!("/notebook/{}/{}/{}", *NAMESPACE, name, p),
            None => format!("/notebook/{}/{}", *NAMESPACE, name),
        }
    }

    impl From<&User> for UserNotebook {
        fn from(user: &User) -> Self {
            let notebook_info = match &user.notebook {
                Some(notebook) => notebook,
                None => &NotebookInfo {
                    ..Default::default()
                },
            };

            let already_visited = user
                .notebook
                .as_ref()
                .map(|nb| nb.last_active.is_some())
                .unwrap_or(false);

            UserNotebook {
                url: make_path(&make_notebook_name(&user._id.to_string()), None),
                name: user._id.to_string(),
                start_time: notebook_info
                    .start_time
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "None".to_string()),
                status: "Pending".to_string(),
                start_up_url: {
                    if already_visited {
                        None
                    } else {
                        notebook_info.start_up_url.clone()
                    }
                },
            }
        }
    }

    /// Consume the bridge cookie and notebook status cookie to create a UserNotebook
    impl From<(BridgeCookie, NotebookStatusCookie)> for UserNotebook {
        fn from(
            (bridge_cookie, notebook_status_cookie): (BridgeCookie, NotebookStatusCookie),
        ) -> Self {
            UserNotebook {
                url: make_path(&make_notebook_name(&bridge_cookie.subject), None),
                name: bridge_cookie.subject,
                start_time: notebook_status_cookie.start_time,
                status: notebook_status_cookie.status,
                start_up_url: notebook_status_cookie.start_url,
            }
        }
    }
}

pub fn config_notebook(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope(&("/notebook/".to_string() + *NAMESPACE))
            .wrap(NotebookCookieCheck)
            .service(notebook_ws_subscribe)
            .service(notebook_ws_session)
            .default_service(web::to(notebook_forward)),
    )
    .service(
        web::scope("/notebook_manage/hx")
            .wrap(CookieCheck)
            .wrap(Htmx)
            .service(notebook_create)
            .service(notebook_delete)
            .service(notebook_status)
            .service(notebook_enter),
    );
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_notebook_forward() {
        // assert!(true);
    }
}
