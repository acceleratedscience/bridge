use std::time;

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
    #[error("{0}")]
    SystemTimeError(#[from] time::SystemTimeError),
    #[error("{0}")]
    JsonWebTokenError(#[from] jsonwebtoken::errors::Error),
    #[error("The query could not be deserialized: {0}")]
    QueryDeserializeError(String),
    #[error("Not admin")]
    NotAdmin,
    #[error("{0}")]
    Unauthorized(String),
}

impl ResponseError for GuardianError {
    fn status_code(&self) -> StatusCode {
        match self {
            GuardianError::GeneralError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::HtmxTagNotFound => StatusCode::BAD_REQUEST,
            GuardianError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::SystemTimeError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::QueryDeserializeError(_) => StatusCode::BAD_REQUEST,
            GuardianError::JsonWebTokenError(e) => {
                match e.kind() {
                    // Unauthorized errors
                    jsonwebtoken::errors::ErrorKind::InvalidToken => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::InvalidSubject => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::RsaFailedSigning => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => StatusCode::UNAUTHORIZED,
                    jsonwebtoken::errors::ErrorKind::ImmatureSignature => StatusCode::UNAUTHORIZED,
                    // Bad request errors
                    jsonwebtoken::errors::ErrorKind::MissingRequiredClaim(_) => {
                        StatusCode::BAD_REQUEST
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidAlgorithmName => {
                        StatusCode::BAD_REQUEST
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidAlgorithm => StatusCode::BAD_REQUEST,
                    jsonwebtoken::errors::ErrorKind::MissingAlgorithm => StatusCode::BAD_REQUEST,
                    jsonwebtoken::errors::ErrorKind::InvalidKeyFormat => StatusCode::BAD_REQUEST,
                    jsonwebtoken::errors::ErrorKind::Json(_) => StatusCode::BAD_REQUEST,
                    // Internal errors
                    jsonwebtoken::errors::ErrorKind::Utf8(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    jsonwebtoken::errors::ErrorKind::Crypto(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    jsonwebtoken::errors::ErrorKind::InvalidEcdsaKey => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidRsaKey(_) => {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                    jsonwebtoken::errors::ErrorKind::Base64(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                }
            }
            GuardianError::NotAdmin => StatusCode::UNAUTHORIZED,
            GuardianError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        }
    }
}
