use std::{fmt::Debug, sync::OnceLock};

use k8s_openapi::{
    api::core::v1::{Namespace, Pod},
    NamespaceResourceScope,
};
use kube::{
    api::{DeleteParams, ObjectMeta, PostParams},
    Api, Client, Resource,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::errors::{GuardianError, Result};

mod models;
pub use models::{Notebook, NotebookSpec, PVCSpec, NAMESPACE};

mod notebook_lifecycle;
pub use notebook_lifecycle::{notebook_lifecycle, LifecycleStream, Medium};

pub struct KubeAPI<M> {
    model: M,
}

static KUBECLIENT: OnceLock<Client> = OnceLock::new();

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
        let res = match crd.create(&pp, &self.model).await {
            Ok(res) => res,
            Err(e) => match e {
                kube::Error::Api(ref error_response) => {
                    if error_response.reason == "AlreadyExists" {
                        return Err(GuardianError::NotebookExistsError(
                            "AlreadyExists".to_string(),
                        ));
                    }
                    Err(e)?
                }
                _ => Err(e)?,
            },
        };
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

    /// Create a namespace if it does not exist. Returns `None` if the namespace already exists.
    /// Returns `Some(())` if the namespace was created.
    pub async fn make_namespace(name: &str) -> Result<Option<()>> {
        let ns = Api::<Namespace>::all(Self::get_kube_client()?.clone());
        if ns.get_opt(name).await?.is_some() {
            return Ok(None);
        }

        let new_ns = Namespace {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            spec: Default::default(),
            status: Default::default(),
        };

        ns.create(&PostParams::default(), &new_ns).await?;
        Ok(Some(()))
    }

    pub async fn check_pod_running(name: &str) -> Result<bool> {
        let pods = Api::<Pod>::namespaced(Self::get_kube_client()?.clone(), NAMESPACE);
        let pod = pods.get(name).await?;
        Ok(pod
            .status
            .as_ref()
            .map(|status| status.phase == Some("Running".to_string()))
            .unwrap_or(false))
    }

    pub async fn get_all_pods() -> Result<Vec<Pod>> {
        let pods = Api::<Pod>::namespaced(Self::get_kube_client()?.clone(), NAMESPACE);
        let list = pods.list(&Default::default()).await?;
        Ok(list.items)
    }

    pub async fn get_pod_ip(name: &str) -> Result<String> {
        let pods = Api::<Pod>::namespaced(Self::get_kube_client()?.clone(), NAMESPACE);
        let pod = pods.get(name).await?;
        Ok(pod
            .status
            .as_ref()
            .and_then(|status| status.pod_ip.as_deref())
            .unwrap_or_default()
            .to_string())
    }
}
