use std::{borrow::Cow, collections::HashSet, marker::PhantomData, str::FromStr, sync::LazyLock};

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
use tracing::instrument;
use url::Url;

use crate::{
    config::CONFIG,
    db::{
        Database,
        models::{BridgeCookie, OWUICookie, OwuiInfo, USER, User},
        mongo::DB,
    },
    errors::{BridgeError, Result},
    kube::{Env, Image, KubeAPI, OpenWebUI, Owui, Persistence},
    web::{
        bson,
        helper::{self, forwarding, observability_post},
    },
};

const OWUI_PORT: &str = "8080";

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
    client: web::Data<reqwest::Client>,
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

    helper::forwarding::forward(
        req,
        payload,
        method,
        peer_addr,
        client,
        url,
        forwarding::Config {
            ..Default::default()
        },
    )
    .await
}

#[post("create")]
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

        let user: User = helper::log_with_level!(
            db.find(
                doc! {
                    "_id": subject,
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
                        name: Cow::from(&kv.0),
                        value: Cow::from(&kv.1),
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
                    "_id": subject,
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
    // data: web::Data<Tera>,
    // ctx: web::Data<Context>,
) -> Result<HttpResponse> {
    if let Some(owui_cookie) = owui_cookie {
        let subject = &owui_cookie.into_inner().subject;
        let name = format!("u{}-openwebui", subject);
        let pvc_name = format!("owui1-{}-openwebui-0", subject);

        let persist_pvc = req.query_string().contains("save");

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
                    "_id": subject,
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

#[inline]
pub(crate) fn make_forward_url(protocol: &str, subject: &str) -> String {
    let namespace = *OWUI_NAMESPACE;
    format!("{protocol}://u{subject}-openwebui.{namespace}.svc.cluster.local:{OWUI_PORT}")
}

pub fn config_openwebui(cfg: &mut web::ServiceConfig) {
    cfg.service(openwebui_ws)
        .default_service(web::to(openwebui_forward));
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
