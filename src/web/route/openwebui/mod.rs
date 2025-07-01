use std::str::FromStr;

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
    db::models::OWUICookie,
    errors::{BridgeError, Result},
    web::helper,
};

const OWUI_PORT: &str = "8080";

#[get("ws/socket.io/")]
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
    url.set_query(Some(req.query_string()));

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
    url.set_path(req.path());
    url.set_query(Some(req.query_string()));

    helper::forwarding::forward(req, payload, method, peer_addr, client, url, None).await
}

#[inline]
pub(crate) fn make_forward_url(protocol: &str, subject: &str) -> String {
    // TODO: see if you can get away without forward slash at the end
    format!("{protocol}://{subject}-openwebui.openwebui.svc.cluster.local:{OWUI_PORT}/")
}

pub fn config_openwebui(cfg: &mut web::ServiceConfig) {
    cfg.service(openwebui_ws)
        .default_service(web::to(openwebui_forward));
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_make_forward_url() {
        let protocol = "http";
        let subject = "test-subject";
        let expected_url =
            format!("{protocol}://{subject}-openwebui.openwebui.svc.cluster.local:8080/");
        assert_eq!(super::make_forward_url(protocol, subject), expected_url);
    }
}
