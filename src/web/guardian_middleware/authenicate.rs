use actix_web::{dev::ServiceRequest, Error};
use actix_web_httpauth::extractors::{
    bearer::{self, BearerAuth},
    AuthenticationError,
};

use crate::web::route::proxy::INFERENCE_HEADER;
use crate::{auth::jwt::validate_token, config::CONFIG};

pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let token = credentials.token().to_string();

    let config = req
        .app_data::<bearer::Config>()
        .cloned()
        .unwrap_or_default()
        .realm("proxy");
    let error = AuthenticationError::from(config);

    match validate_token(
        &token,
        &CONFIG.get().unwrap().decoder,
        &CONFIG.get().unwrap().validation.clone(),
    ) {
        Ok(claims) => {
            if let Some(r) = req.headers().get(INFERENCE_HEADER) {
                // TODO: handle unwrap_or better
                let inference = r.to_str().unwrap_or("");
                if claims.scp.contains(&inference.to_string()) {
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
