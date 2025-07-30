use actix_web::{Error, dev::ServiceRequest};
use actix_web_httpauth::extractors::{
    AuthenticationError,
    bearer::{self, BearerAuth},
};
use tracing::info;

use crate::{
    auth::jwt::validate_token,
    config::CONFIG,
    logger::{PERSIST_META, PERSIST_TIME},
    web::route::proxy::INFERENCE_HEADER,
};

pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let token = credentials.token();

    let config = req
        .app_data::<bearer::Config>()
        .cloned()
        .unwrap_or_default()
        .realm("proxy");
    let error = AuthenticationError::from(config);

    match validate_token(token, &CONFIG.decoder, &CONFIG.validation) {
        Ok(claims) => {
            if let Some(r) = req.headers().get(INFERENCE_HEADER) {
                // TODO: handle unwrap_or better
                let inference = r.to_str().unwrap_or("");
                if claims.scp.contains(&inference.to_string()) {
                    let now = time::OffsetDateTime::now_utc().unix_timestamp();
                    info!(target: PERSIST_META,
                        sub=claims.get_sub(), property=inference, request_date=now, expire_soon_after=now+PERSIST_TIME);

                    return Ok(req);
                }
            }
            Err((
                error
                    .with_error_description("Inference-Service Issue")
                    .into(),
                req,
            ))
        }
        Err(_e) => {
            // TODO: handler the error better
            Err((error.into(), req))
        }
    }
}
