pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
// feature notebook or openwebui
#[cfg(any(feature = "notebook", feature = "openwebui"))]
pub mod kube;
pub mod logger;
pub mod templating;
pub mod web;
