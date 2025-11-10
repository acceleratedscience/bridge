use actix_web::{HttpRequest, HttpResponse, dev::PeerAddr, http::Method, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
#[cfg(feature = "observe")]
use tracing::info;
use tracing::{instrument, warn};

#[cfg(feature = "observe")]
use crate::logger::{PERSIST_META, PERSIST_TIME};
use crate::{
    auth::jwt::validate_token,
    config::CONFIG,
    errors::{BridgeError, Result},
    web::{
        helper::{self, forwarding::Config},
        services::CATALOG,
    },
};

const MCP_PREFIX: &str = "/mcp/";
const MCP_SUFFIX: &str = "-s";

#[instrument(skip(payload))]
async fn forward(
    req: HttpRequest,
    payload: web::Payload,
    credentials: BearerAuth,
    method: Method,
    peer_addr: Option<PeerAddr>,
    client: web::Data<reqwest::Client>,
) -> Result<HttpResponse> {
    let token = credentials.token();
    let path = req
        .uri()
        .path()
        .strip_prefix(MCP_PREFIX)
        .unwrap_or(req.uri().path());

    if let Some(mcp) = path.split('/').next() {
        let path = path.strip_prefix(mcp).unwrap_or(path);

        if !CATALOG.is_service_mcp(mcp)? {
            return Err(BridgeError::ServiceDoesNotExist(mcp.to_string()));
        }

        if let Ok(claims) = validate_token(token, &CONFIG.decoder, &CONFIG.validation) {
            // ugh TODO: Remove this extra string clone
            if !claims.scp.contains(&mcp.to_string()) {
                return Err(BridgeError::Unauthorized(format!(
                    "User does not have access to {mcp}"
                )));
            }

            #[cfg(feature = "observe")]
            {
                let now = time::OffsetDateTime::now_utc().unix_timestamp();
                info!(target: PERSIST_META, 
                sub=claims.get_sub(), property=mcp, request_date=now, expire_soon_after=now+PERSIST_TIME);
            }
        } else {
            return Err(BridgeError::Unauthorized(
                "JWT token is invalid".to_string(),
            ));
        }

        let mut new_url = helper::log_with_level!(CATALOG.get_service(mcp), error)?;
        // fastmcp temp(?) fix
        if mcp.ends_with(MCP_SUFFIX) {
            new_url.set_path(&format!("{path}/"));
        } else {
            new_url.set_path(path);
        }
        new_url.set_query(req.uri().query());

        helper::forwarding::forward(
            req,
            payload,
            method,
            peer_addr,
            client,
            new_url,
            Config {
                inference: true,
                ..Default::default()
            },
        )
        .await
    } else {
        warn!("MCP service not found in url request");
        Err(BridgeError::MCPParseIssue)
    }

    // get header for which infernce service to forward to
}

pub fn config_mcp(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/mcp").default_service(web::to(forward)));
}
