use actix_web::{
    HttpRequest, HttpResponse, get,
    web::{self, Data, ReqData},
};
use tracing::instrument;

use crate::{config::CacheDB, db::models::BridgeCookie, errors::Result};

#[get("/me")]
#[instrument(skip(_cache))]
async fn me(
    req: HttpRequest,
    data: Option<ReqData<BridgeCookie>>,
    _cache: Data<Option<&CacheDB>>,
) -> Result<HttpResponse> {
    let cookie = match data {
        Some(data) => data.into_inner(),
        None => {
            return Ok(HttpResponse::Unauthorized().finish());
        }
    };

    let json = serde_json::to_string(&cookie)?;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(json))
}

pub fn config_api(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/auth").service(me));
}
