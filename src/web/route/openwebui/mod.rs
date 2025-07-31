use std::{collections::HashSet, str::FromStr, sync::LazyLock};

use actix_web::{
    HttpRequest, HttpResponse,
    dev::PeerAddr,
    get,
    http::Method,
    web::{self, ReqData},
};
use tracing::instrument;
use url::Url;

use crate::{
    config::CONFIG,
    db::models::OWUICookie,
    errors::{BridgeError, Result},
    web::helper,
};

const OWUI_PORT: &str = "8080";
const MOLE_VIEW_PORT: &str = "8024";

pub static OWUI_NAMESPACE: LazyLock<&str> = LazyLock::new(|| &CONFIG.owui_namespace);
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

    let mut url = Url::from_str(&make_forward_url("ws", &owui_cookie.subject, false))?;
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

    let mut url = Url::from_str(&make_forward_url("http", &owui_cookie.subject, false))?;
    let path = req.path();
    url.set_path(path);

    if WHITELIST_ENDPOINTS.contains(path)
        && let Ok(ref mut p) = url.path_segments_mut()
    {
        p.push("");
    }

    url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, url, None, false).await
}

#[instrument(skip(payload))]
async fn moleviewer_forward(
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

    let mut url = Url::from_str(&make_forward_url("http", &owui_cookie.subject, true))?;
    let path = req.path();
    url.set_path(path);
    url.set_query(req.uri().query());

    helper::forwarding::forward(req, payload, method, peer_addr, client, url, None, false).await
}

#[inline]
pub(crate) fn make_forward_url(protocol: &str, subject: &str, mole: bool) -> String {
    let namespace = *OWUI_NAMESPACE;
    let port = if mole { MOLE_VIEW_PORT } else { OWUI_PORT };
    format!("{protocol}://u{subject}-openwebui.{namespace}.svc.cluster.local:{port}")
}

pub fn config_openwebui(cfg: &mut web::ServiceConfig) {
    cfg.service(openwebui_ws)
        .default_service(web::to(openwebui_forward));
}

pub fn config_moleviewer(cfg: &mut web::ServiceConfig) {
    cfg.default_service(web::to(moleviewer_forward));
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
        assert_eq!(
            super::make_forward_url(protocol, subject, false),
            expected_url
        );
    }
}
