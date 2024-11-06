use std::{fmt::Debug, sync::OnceLock};

use k8s_openapi::NamespaceResourceScope;
use kube::{
    api::{DeleteParams, PostParams},
    Api, Client, Resource,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::errors::Result;

mod models;
pub use models::{Notebook, NotebookSpec};

#[allow(dead_code)]
pub struct KubeAPI<M> {
    client: Client,
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
    M: Resource + Clone + Debug + for<'a> Deserialize<'a> + Serialize,
    M: Resource<Scope = NamespaceResourceScope>,
    <M as Resource>::DynamicType: Default,
{
    pub fn new(model: M) -> Self {
        Self {
            client: {
                let Some(client) = KUBECLIENT.get() else {
                    unreachable!(
                        "K8s client not initialized... should never happen as long as init_once is called first"
                    );
                };
                client.clone()
            },
            model,
        }
    }

    pub async fn create(&self) -> Result<M> {
        let crd = Api::<M>::namespaced(self.client.clone(), "guardian");
        let pp = PostParams::default();
        let res = crd.create(&pp, &self.model).await?;
        Ok(res)
    }

    pub async fn delete(&self, name: &str) -> Result<StatusCode> {
        let crd = Api::<M>::namespaced(self.client.clone(), "guardian");
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
