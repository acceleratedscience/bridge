mod authenicate;
mod cookie_check;
mod errors;
mod htmx;
mod https;
mod maintenance;
#[cfg(feature = "notebook")]
mod notebook_cookie_check;
mod resource_cookie_check;
mod security_cache_header;
#[cfg(feature = "openwebui")]
mod openwebui_cookie_check;

pub use authenicate::validator;
pub use cookie_check::CookieCheck;
pub use errors::custom_code_handle;
pub use htmx::{HTMX_ERROR_RES, Htmx};
pub use maintenance::{MAINTENANCE_WINDOWS, Maintainence};
#[cfg(feature = "notebook")]
pub use notebook_cookie_check::NotebookCookieCheck;
pub use resource_cookie_check::ResourceCookieCheck;
pub use security_cache_header::SecurityCacheHeader;
#[cfg(feature = "openwebui")]
pub use openwebui_cookie_check::OWUICookieCheck;

#[allow(unused_imports)]
pub use https::HttpRedirect;
