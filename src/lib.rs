pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
// TODO: change to notebook to kubernetes
#[cfg(feature = "kubernetes")]
pub mod kube;
pub mod logger;
pub mod templating;
pub mod web;
