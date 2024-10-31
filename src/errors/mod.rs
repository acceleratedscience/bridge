use std::convert::Infallible;

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
    SystemTimeError(#[from] std::time::SystemTimeError),
    #[error("{0}")]
    JsonWebTokenError(#[from] jsonwebtoken::errors::Error),
    #[error("The query could not be deserialized: {0}")]
    QueryDeserializeError(String),
    #[error("Not admin")]
    NotAdmin,
    #[error("{0}")]
    Unauthorized(String),
    #[error("Inference-Service header not found")]
    InferenceServiceHeaderNotFound,
    #[error("Service {0} does does not exist")]
    ServiceDoesNotExist(String),
    #[error("Toml lookup error")]
    TomlLookupError,
    #[error("{0}")]
    TomlError(#[from] toml::de::Error),
    #[error("String conversion error")]
    StringConversionError,
    #[error("{0}")]
    DeserializationError(#[from] serde::de::value::Error),
    #[error("{0}")]
    ClaimsVerificationError(#[from] openidconnect::ClaimsVerificationError),
    #[error("Nonce cookie not found")]
    NonceCookieNotFound,
    #[error("Error when requesting token from Auth server: {0}")]
    TokenRequestError(String),
    #[error("{0}")]
    IOError(#[from] std::io::Error),
    #[error("{0}")]
    URLParseError(#[from] url::ParseError),
    #[error("Authorization server not supported")]
    AuthorizationServerNotSupported,
    #[error("{0}")]
    MongoError(#[from] mongodb::error::Error),
    #[error("{0}")]
    UserNotFound(String),
    #[error("User cannot access this page {0}")]
    UserNotAllowedOnPage(String),
    #[error("{0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("{0}")]
    FormDeserializeError(String),
    #[error("{0}")]
    RecordSearchError(String),
    #[error("{0}")]
    WSError(#[from] actix_web::error::Error),
    #[error("{0}")]
    KubeError(#[from] kube::Error),
    #[error("{0}")]
    NotebookExistsError(String),
    #[error("{0}")]
    NotebookAccessError(String),
}

// Workaround for Infallible, which may get solved by rust-lang: https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for GuardianError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl ResponseError for GuardianError {
    fn status_code(&self) -> StatusCode {
        match self {
            GuardianError::GeneralError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::SystemTimeError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::QueryDeserializeError(_) => StatusCode::BAD_REQUEST,
            GuardianError::InferenceServiceHeaderNotFound => StatusCode::BAD_REQUEST,
            GuardianError::ServiceDoesNotExist(_) => StatusCode::BAD_REQUEST,
            GuardianError::HtmxTagNotFound => StatusCode::BAD_REQUEST,
            GuardianError::AuthorizationServerNotSupported => StatusCode::BAD_REQUEST,
            GuardianError::FormDeserializeError(_) => StatusCode::BAD_REQUEST,
            GuardianError::RecordSearchError(_) => StatusCode::BAD_REQUEST,
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
            GuardianError::TomlLookupError => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::StringConversionError => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::DeserializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::IOError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::TomlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::URLParseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::MongoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::SerdeJsonError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::WSError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GuardianError::KubeError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            GuardianError::NotAdmin => StatusCode::UNAUTHORIZED,
            GuardianError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            GuardianError::ClaimsVerificationError(_) => StatusCode::UNAUTHORIZED,
            GuardianError::NonceCookieNotFound => StatusCode::UNAUTHORIZED,
            GuardianError::TokenRequestError(_) => StatusCode::UNAUTHORIZED,

            GuardianError::UserNotFound(_) => StatusCode::FORBIDDEN,
            GuardianError::UserNotAllowedOnPage(_) => StatusCode::FORBIDDEN,
            GuardianError::NotebookAccessError(_) => StatusCode::FORBIDDEN,

            GuardianError::NotebookExistsError(_) => StatusCode::CONFLICT,
        }
    }
}
