use std::marker::PhantomData;

use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite},
    dev::PeerAddr,
    get,
    http::Method,
    web::{self, Data, ReqData},
};
use tracing::instrument;

use crate::{
    auth::{COOKIE_NAME, jwt},
    config::{AUD, CONFIG},
    db::{
        Database,
        models::{BridgeCookie, USER, User},
        mongo::DB,
    },
    errors::{BridgeError, Result},
    web::{
        bridge_middleware::ResourceCookieCheck,
        helper::{self, forwarding::Config},
        services::CATALOG,
    },
};

static TOKEN_LIFETIME: usize = 60 * 60 * 24; // 24 hours

#[instrument(skip(payload, db))]
async fn resource_http(
    req: HttpRequest,
    payload: web::Payload,
    db: Data<&DB>,
    resource: ReqData<(BridgeCookie, String)>,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let (mut bridge_cookie, resource) = resource.into_inner();
    let prefix = format!("/resource/{}", &resource);
    let path = req
        .uri()
        .path()
        .strip_prefix(&prefix)
        .unwrap_or(req.uri().path());
    // check query for token=true
    let updated_cookie = if req.uri().query().is_some_and(|q| q.contains("token=true")) {
        let pipeline = db.get_user_group_pipeline(&bridge_cookie.subject);
        let groups = db.aggregate(pipeline, USER, PhantomData::<User>).await?;
        let scp = groups
            .into_iter()
            .next()
            .map(|gs| gs.group_subscriptions)
            .and_then(|gs| gs.into_iter().next())
            .unwrap_or_default();

        let (token, _) = jwt::get_token_and_exp(
            &CONFIG.encoder,
            TOKEN_LIFETIME,
            &bridge_cookie.subject,
            AUD[0],
            scp,
        )?;

        bridge_cookie.token = Some(token);
        let content = serde_json::to_string(&bridge_cookie).map_err(|e| {
            BridgeError::GeneralError(format!("Could not serialize bridge cookie: {e}"))
        })?;
        Some(
            Cookie::build(COOKIE_NAME, content)
                .same_site(SameSite::Strict)
                .expires(time::OffsetDateTime::now_utc() + time::Duration::days(1))
                .path("/")
                .http_only(true)
                .secure(true)
                .finish(),
        )
    } else {
        None
    };

    let mut new_url = helper::log_with_level!(CATALOG.get_resource(&resource), error)?;
    new_url.set_path(path);
    new_url.set_query(req.uri().query());

    helper::forwarding::forward(
        req,
        payload,
        method,
        peer_addr,
        client,
        new_url,
        Config {
            updated_cookie,
            pack_cookies: true,
            ..Default::default()
        },
    )
    .await
}

#[instrument(skip(pl))]
#[get("{resource_name}/ws/{path:.*}")]
async fn resource_ws(
    req: HttpRequest,
    pl: web::Payload,
    resource: ReqData<(BridgeCookie, String)>,
    webpath: web::Path<(String, String)>,
) -> Result<HttpResponse> {
    let (_, resource) = resource.into_inner();
    let (_, path) = webpath.into_inner();

    let mut new_url = helper::log_with_level!(CATALOG.get_resource(&resource), error)?;

    helper::log_with_level!(
        new_url
            .set_scheme("ws")
            .map_err(|_| BridgeError::GeneralError("Could not set scheme to ws".to_string())),
        error
    )?;

    new_url.set_path(&path);
    new_url.set_query(req.uri().query());

    helper::ws::manage_connection(req, pl, new_url).await
}

#[instrument(skip(pl))]
#[get("{resource_name}/wss")]
async fn resource_wss(
    req: HttpRequest,
    pl: web::Payload,
    resource: ReqData<(BridgeCookie, String)>,
    webpath: web::Path<String>,
) -> Result<HttpResponse> {
    let (_, resource) = resource.into_inner();

    let mut new_url = helper::log_with_level!(CATALOG.get_resource(&resource), error)?;

    helper::log_with_level!(
        new_url
            .set_scheme("wss")
            .map_err(|_| BridgeError::GeneralError("Could not set scheme to ws".to_string())),
        error
    )?;

    new_url.set_query(req.uri().query());

    helper::ws::manage_connection(req, pl, new_url).await
}

pub fn config_resource(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/resource")
            .wrap(ResourceCookieCheck)
            .service(resource_wss)
            .service(resource_ws)
            .default_service(web::to(resource_http)),
    );
}
