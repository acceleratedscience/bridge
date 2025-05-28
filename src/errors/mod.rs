use std::convert::Infallible;
#[cfg(feature = "observe")]
use std::sync::mpsc;

use actix_web::{ResponseError, http::StatusCode};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BridgeError>;

#[derive(Error, Debug)]
pub enum BridgeError {
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
    #[error("{0}")]
    IntrospectionError(&'static str),
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
    #[cfg(feature = "notebook")]
    #[error("{0}")]
    KubeError(#[from] kube::Error),
    #[cfg(feature = "notebook")]
    #[error("{0}")]
    NotebookExistsError(String),
    #[cfg(feature = "notebook")]
    #[error("{0}")]
    NotebookAccessError(String),
    #[cfg(feature = "notebook")]
    #[error("{0}")]
    #[cfg(feature = "notebook")]
    KubeClientError(String),
    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("{0}")]
    RedisError(#[from] redis::RedisError),
    #[error("{0}")]
    OpenIDError(#[from] openidconnect::ConfigurationError),
    #[error("{0}")]
    Argon2Error(#[from] argon2::Error),
    #[error("Cannot parse MCP from URL")]
    #[cfg(feature = "mcp")]
    MCPParseIssue,
    #[error("{0}")]
    #[cfg(feature = "observe")]
    MSGSendError(#[from] mpsc::SendError<String>),
    #[error("{0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

// Workaround for Infallible, which may get solved by rust-lang: https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for BridgeError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl ResponseError for BridgeError {
    fn status_code(&self) -> StatusCode {
        match self {
            BridgeError::GeneralError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::SystemTimeError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "observe")]
            BridgeError::MSGSendError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::TokioJoinError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            BridgeError::QueryDeserializeError(_) => StatusCode::BAD_REQUEST,
            BridgeError::InferenceServiceHeaderNotFound => StatusCode::BAD_REQUEST,
            BridgeError::ServiceDoesNotExist(_) => StatusCode::BAD_REQUEST,
            BridgeError::HtmxTagNotFound => StatusCode::BAD_REQUEST,
            BridgeError::AuthorizationServerNotSupported => StatusCode::BAD_REQUEST,
            BridgeError::FormDeserializeError(_) => StatusCode::BAD_REQUEST,
            BridgeError::RecordSearchError(_) => StatusCode::BAD_REQUEST,
            BridgeError::IntrospectionError(_) => StatusCode::BAD_REQUEST,
            #[cfg(feature = "mcp")]
            BridgeError::MCPParseIssue => StatusCode::BAD_REQUEST,

            BridgeError::JsonWebTokenError(e) => {
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
            BridgeError::TomlLookupError => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::StringConversionError => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::DeserializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::IOError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::TomlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::URLParseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::MongoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::SerdeJsonError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::WSError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::ReqwestError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::RedisError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::OpenIDError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Argon2Error(_) => StatusCode::INTERNAL_SERVER_ERROR,

            #[cfg(feature = "notebook")]
            BridgeError::KubeError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "notebook")]
            BridgeError::KubeClientError(_) => StatusCode::INTERNAL_SERVER_ERROR,

            BridgeError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            BridgeError::ClaimsVerificationError(_) => StatusCode::UNAUTHORIZED,
            BridgeError::NonceCookieNotFound => StatusCode::UNAUTHORIZED,
            BridgeError::TokenRequestError(_) => StatusCode::UNAUTHORIZED,
            BridgeError::UserNotAllowedOnPage(_) => StatusCode::UNAUTHORIZED,

            BridgeError::UserNotFound(_) => StatusCode::FORBIDDEN,
            BridgeError::NotAdmin => StatusCode::FORBIDDEN,
            #[cfg(feature = "notebook")]
            BridgeError::NotebookAccessError(_) => StatusCode::FORBIDDEN,

            #[cfg(feature = "notebook")]
            BridgeError::NotebookExistsError(_) => StatusCode::CONFLICT,
        }
    }
}
