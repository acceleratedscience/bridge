use std::{
    borrow::Cow, collections::HashSet, marker::PhantomData, str::FromStr, sync::LazyLock,
    time::Duration,
};

use actix_web::{
    HttpRequest, HttpResponse, delete,
    dev::PeerAddr,
    get,
    http::{Method, header::ContentType},
    post,
    web::{self, ReqData},
};
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use kube::api::ObjectMeta;
use mongodb::bson::doc;
use tera::{Context, Tera};
use tracing::{instrument, warn};
use url::Url;

use crate::{
    config::CONFIG,
    db::{
        Database,
        models::{BridgeCookie, OWUICookie, OwuiInfo, USER, User, UserOwui},
        mongo::{DB, ObjectID},
    },
    errors::{BridgeError, Result},
    kube::{Env, Image, KubeAPI, OpenWebUI, Owui, Persistence},
    web::{
        bridge_middleware::{CookieCheck, Htmx},
        bson,
        helper::{self, Config, observability_post},
        proxy_client::{self, ProxyClient},
    },
};

const OWUI_PORT: &str = "8080";
const PVC_DELETE_ATTEMPT: u8 = 9;

pub static OWUI_NAMESPACE: LazyLock<&str> = LazyLock::new(|| &CONFIG.owui.namespace);
static WHITELIST_ENDPOINTS: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    HashSet::from([
        "/api/v1/auths",
        "/api/v1/channels",
        "/api/v1/folders",
        "/api/v1/tools",
        "/api/v1/chats",
        "/api/v1/knowledge",
        "/api/v1/models",
        "/api/v1/groups",
        "/api/v1/prompts",
        "/api/v1/users",
        "/api/v1/functions",
        "/api/v1/files",
    ])
});

#[get("ws/socket.io")]
async fn openwebui_ws(
    req: HttpRequest,
    pl: web::Payload,
    owiu_cookie: Option<ReqData<OWUICookie>>,
) -> Result<HttpResponse> {
    let owui_cookie = match owiu_cookie {
        Some(cookie) => cookie.into_inner(),
        None => {
            return Err(BridgeError::Unauthorized(
                "OWUI cookie not found".to_string(),
            ));
        }
    };

    let mut url = Url::from_str(&make_forward_url("ws", &owui_cookie.subject))?;
    url.set_path("ws/socket.io/");
    url.set_query(req.uri().query());

    helper::ws::manage_connection(req, pl, url).await
}

#[instrument(skip(payload))]
async fn openwebui_forward(
    req: HttpRequest,
    payload: web::Payload,
    method: Method,
    peer_addr: Option<PeerAddr>,
    owui_cookie: Option<ReqData<OWUICookie>>,
    client: web::Data<ProxyClient>,
) -> Result<HttpResponse> {
    let owui_cookie = match owui_cookie {
        Some(cookie) => cookie.into_inner(),
        None => {
            return Err(BridgeError::Unauthorized(
                "OWUI cookie not found".to_string(),
            ));
        }
    };

    let mut url = Url::from_str(&make_forward_url("http", &owui_cookie.subject))?;
    let path = req.path();
    url.set_path(path);

    if WHITELIST_ENDPOINTS.contains(path)
        && let Ok(ref mut p) = url.path_segments_mut()
    {
        p.push("");
    }

    url.set_query(req.uri().query());

    proxy_client::forward(
        req,
        payload,
        method,
        peer_addr,
        client,
        url,
        Config {
            ..Default::default()
        },
    )
    .await
}

#[post("/create")]
async fn create_owui(
    req: HttpRequest,
    owui_cookie: Option<ReqData<OWUICookie>>,
    bcookie: Option<ReqData<BridgeCookie>>,
    db: web::Data<&DB>,
    data: web::Data<Tera>,
    ctx: web::Data<Context>,
) -> Result<HttpResponse> {
    if let Some(owui_cookie) = owui_cookie {
        let subject = &owui_cookie.into_inner().subject;
        let name = format!("u{}-openwebui", subject);
        let pvc_name = format!("owui1-u{}-openwebui-0", subject);

        let user: User = helper::log_with_level!(
            db.find(
                doc! {
                    "_id": ObjectID::new(subject).into_inner(),
                },
                USER,
            )
            .await,
            error
        )?;

        // check if an instance alreadu exists
        let list_owui = KubeAPI::<Owui>::get_crds(&CONFIG.owui.namespace).await?;
        if list_owui
            .iter()
            .any(|o| o.metadata.name.as_ref().unwrap_or(&"".to_string()) == &name)
        {
            return Err(BridgeError::CRDExistsError(format!(
                "OWUI instance already exists for user {}",
                name
            )));
        }

        if req.query_string().contains("clear") {
            helper::log_with_level!(
                KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name, &CONFIG.owui.namespace).await,
                error
            )?;

            // PVC takes time to delete... loop and check it is gone
            loop {
                let mut loop_cnt = 0;
                if KubeAPI::<PersistentVolumeClaim>::check_pvc_exists(
                    &pvc_name,
                    &CONFIG.owui.namespace,
                )
                .await?
                {
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

        // let persist = req
        //     .uri()
        //     .query()
        //     .map(|q| q.contains("persist=true"))
        //     .unwrap_or(false);

        // create instance
        let owui = Owui {
            metadata: ObjectMeta {
                name: Some(name),
                namespace: Some(CONFIG.owui.namespace.clone()),
                ..Default::default()
            },
            spec: OpenWebUI {
                replica: 1, // TODO: this needs to be removed from the operator.., for now set to 1
                retain_pvc: true,
                service_port: CONFIG.owui.service_port,
                image: Image {
                    registry: Cow::from(&CONFIG.owui.registry),
                    repository: Cow::from(&CONFIG.owui.repository),
                    tag: Cow::from(&CONFIG.owui.tag),
                    pull_policy: Cow::from(&CONFIG.owui.pull_policy),
                },
                persistence: Persistence {
                    size: Cow::from(&CONFIG.owui.persistence_size),
                    storage_class: Cow::from(&CONFIG.owui.persistence_storage_class),
                },
                env: CONFIG
                    .owui
                    .env
                    .iter()
                    .map(|kv| Env {
                        name: Cow::from(kv.0.trim_matches('"')),
                        value: Cow::from(kv.1.trim_matches('"')),
                    })
                    .collect(),
            },
        };
        if let Err(e) = KubeAPI::new(owui).create(&CONFIG.owui.namespace).await {
            return helper::log_with_level!(
                Err(BridgeError::GeneralError(format!(
                    "Failed to create OWUI instance: {e}"
                ))),
                error
            );
        }

        // update DB with instance information
        let current_time = time::OffsetDateTime::now_utc();
        let _r = db
            .update(
                doc! {
                    "_id": ObjectID::new(subject).into_inner(),
                },
                doc! {
                    "$set": doc! {
                        "updated_at": bson(current_time)?,
                        "owui": bson(OwuiInfo{
                            start_time: Some(current_time),
                            last_active: None,
                        })?,
                        "last_updated_by": &user.sub,
                    },
                },
                USER,
                PhantomData::<User>,
            )
            .await?;

        if let Some(bc) = bcookie {
            observability_post("owui instance has been created", &bc);
        }

        let context = data.render("components/owui/poll.html", &ctx)?;
        return Ok(HttpResponse::Ok()
            .content_type(ContentType::form_url_encoded())
            .body(context));
    }
    helper::log_with_level!(
        Err(BridgeError::Forbidden(
            "User does not have access to OWUI... middleware should have prevented this"
                .to_string()
        )),
        error
    )
}

#[delete("delete")]
async fn delete_owui(
    req: HttpRequest,
    // payload: web::Payload,
    // method: Method,
    // peer_addr: Option<PeerAddr>,
    owui_cookie: Option<ReqData<OWUICookie>>,
    bcookie: Option<ReqData<BridgeCookie>>,
    db: web::Data<&DB>,
    data: web::Data<Tera>,
    ctx: web::Data<Context>,
) -> Result<HttpResponse> {
    if let Some(owui_cookie) = owui_cookie {
        let subject = &owui_cookie.into_inner().subject;
        let name = format!("u{}-openwebui", subject);
        let pvc_name = format!("owui1-u{}-openwebui-0", subject);

        let mut ctx = (**ctx).clone();

        let persist_pvc = req.query_string().contains("save");

        if persist_pvc {
            ctx.insert("pvc_exists_owui", &persist_pvc);
        }

        helper::log_with_level!(
            KubeAPI::<Owui>::delete(&name, &CONFIG.owui.namespace).await,
            error
        )?;

        if !persist_pvc {
            // don't stop due to error here so we can remove the rest
            let _ = helper::log_with_level!(
                KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name, &CONFIG.owui.namespace).await,
                error
            );
        }

        let current_time = time::OffsetDateTime::now_utc();
        let _r = db
            .update(
                doc! {
                    "_id": ObjectID::new(subject).into_inner(),
                },
                doc! {
                    "$set": doc! {
                        "updated_at": bson(current_time)?,
                        "owui": null,
                    },
                },
                USER,
                PhantomData::<User>,
            )
            .await?;

        if let Some(bc) = bcookie {
            observability_post("owui instance has been deleted", &bc);
        }

        ctx.insert("cooloff", &true);
        let context = data.render("components/owui/start.html", &ctx)?;

        return Ok(HttpResponse::Ok()
            .content_type(ContentType::form_url_encoded())
            .body(context));
    }
    helper::log_with_level!(
        Err(BridgeError::Forbidden(
            "User does not have access to OWUI... middleware should have prevented this"
                .to_string()
        )),
        error
    )
}

#[get("status")]
async fn status_owui(
    owui_cookie: Option<ReqData<OWUICookie>>,
    data: web::Data<Tera>,
    ctx: web::Data<Context>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    if let Some(owui_cookie) = owui_cookie {
        let subject = &owui_cookie.into_inner().subject;
        let url = make_forward_url("http", subject);

        if !client.get(url).send().await?.status().is_success() {
            return Ok(HttpResponse::ServiceUnavailable().finish());
        }

        let mut ctx = (**ctx).clone();

        ctx.insert("owui_subject", subject);
        ctx.insert("owui_url", &CONFIG.owui.url);
        let content = data.render("components/owui/ready.html", &ctx)?;

        return Ok(HttpResponse::Ok()
            .content_type(ContentType::form_url_encoded())
            .body(content));
    }
    helper::log_with_level!(
        Err(BridgeError::Forbidden(
            "User does not have access to OWUI... middleware should have prevented this"
                .to_string()
        )),
        error
    )
}

impl From<&User> for UserOwui {
    fn from(value: &User) -> Self {
        Self {
            start_time: value
                .owui
                .as_ref()
                .map(|v| v.start_time.unwrap_or(time::OffsetDateTime::now_utc()))
                .map(|v| v.to_string())
                .unwrap_or_default(),
            name: value.sub.to_owned(),
            status: "Pending".to_string(),
        }
    }
}

#[inline]
pub(crate) fn make_forward_url(protocol: &str, subject: &str) -> String {
    let namespace = *OWUI_NAMESPACE;
    // if in dev mode
    // if cfg!(debug_assertions) {
    //     return format!("{protocol}://0.0.0.0:{OWUI_PORT}");
    // }
    format!("{protocol}://u{subject}-openwebui.{namespace}.svc.cluster.local:{OWUI_PORT}")
}

pub fn config_openwebui(cfg: &mut web::ServiceConfig) {
    cfg.service(openwebui_ws)
        .default_service(web::to(openwebui_forward));
}

pub fn config_openwebui_manage(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/owui_manage/hx")
            .wrap(CookieCheck)
            .wrap(Htmx)
            .service(create_owui)
            .service(delete_owui)
            .service(status_owui),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_make_forward_url() {
        // kind of a silly test; can be used as a contract on how url is set
        let protocol = "http";
        let subject = "test-subject";
        let namespace = "openwebui";
        let port = "8080";
        let expected_url =
            format!("{protocol}://u{subject}-openwebui.{namespace}.svc.cluster.local:{port}");
        assert_eq!(super::make_forward_url(protocol, subject), expected_url);
    }
}
