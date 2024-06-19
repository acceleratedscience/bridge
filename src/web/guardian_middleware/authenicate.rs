use actix_web::dev::ServiceRequest;
use actix_web::Error;
use actix_web_httpauth::extractors::bearer::{self, BearerAuth};
use actix_web_httpauth::extractors::AuthenticationError;

use crate::{auth::jwt::validate_token, config::CONFIG};

pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let token = credentials.token().to_string();
    match validate_token(
        &token,
        &CONFIG.get().unwrap().decoder,
        &CONFIG.get().unwrap().validation.clone(),
    ) {
        Ok(_) => Ok(req),
        Err(_e) => {
            // TODO: handler the error better
            let config = req.app_data::<bearer::Config>().cloned().unwrap_or_default().realm("proxy");
            Err((AuthenticationError::from(config).into(), req))
        }
    }
}
