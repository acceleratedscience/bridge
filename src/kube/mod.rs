use std::{fmt::Debug, sync::OnceLock};

use k8s_openapi::NamespaceResourceScope;
use kube::{
    api::{DeleteParams, PostParams},
    Api, Client, Resource,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::errors::{GuardianError, Result};

mod models;
pub use models::{Notebook, NotebookSpec, PVCSpec};

#[allow(dead_code)]
pub struct KubeAPI<M> {
    model: M,
}

static KUBECLIENT: OnceLock<Client> = OnceLock::new();
const NAMESPACE: &str = "guardian";

pub async fn init_once() {
    // ok to fail since we should not start if we can't connect to k8s
    let client = Client::try_default()
        .await
        .expect("Failed to connect to k8s");
    info!("Connected to k8s");
    KUBECLIENT.get_or_init(|| client);
}

impl<M> KubeAPI<M>
where
    M: Resource<Scope = NamespaceResourceScope>
        + Clone
        + Debug
        + for<'a> Deserialize<'a> // hrtb
        + Serialize,
    <M as Resource>::DynamicType: Default,
{
    pub fn new(model: M) -> Self {
        Self { model }
    }

    pub fn get_kube_client() -> Result<&'static Client> {
        KUBECLIENT.get().ok_or(GuardianError::KubeClientError(
            "Could not get kube client".to_string(),
        ))
    }

    pub async fn create(&self) -> Result<M> {
        let crd = Api::<M>::namespaced(Self::get_kube_client()?.clone(), NAMESPACE);
        let pp = PostParams::default();
        let res = crd.create(&pp, &self.model).await?;
        Ok(res)
    }

    pub async fn delete(name: &str) -> Result<StatusCode> {
        let crd = Api::<M>::namespaced(Self::get_kube_client()?.clone(), NAMESPACE);
        let dp = DeleteParams::default();
        let status = match crd.delete(name, &dp).await? {
            // resource is in the process of being deleted
            either::Either::Left(_) => StatusCode::PROCESSING,
            // resource has been deleted
            either::Either::Right(_) => StatusCode::OK,
        };
        Ok(status)
    }
}
