use actix_web::{http::StatusCode, ResponseError};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GuardianError>;

#[derive(Error, Debug)]
pub enum GuardianError {
    #[error("{0}")]
    GeneralError(String),
    #[error("HTMX tag not found in header")]
    HtmxTagNotFound,
    #[error("{0}")]
    TeraError(#[from] tera::Error),
}

impl ResponseError for GuardianError {
    fn status_code(&self) -> StatusCode {
        match self {
            GuardianError::GeneralError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::HtmxTagNotFound => StatusCode::BAD_REQUEST,
            GuardianError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
