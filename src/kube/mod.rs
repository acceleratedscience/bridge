mod models;
pub use models::*;

#[cfg(feature = "notebook")]
mod notebook;
#[cfg(feature = "notebook")]
pub use notebook::{
    LifecycleStream, Medium, NOTEBOOK_NAMESPACE, Notebook, NotebookSpec, PVCSpec,
    notebook_lifecycle,
};

#[cfg(feature = "openwebui")]
mod openwebui;
