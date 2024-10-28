use std::{fmt::Debug, sync::OnceLock};

use k8s_openapi::{Metadata, Resource};
use kube::{
    api::{DeleteParams, ObjectMeta, PostParams},
    Api, Client,
};
use serde::{Deserialize, Serialize};

use crate::errors::Result;

mod models;

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
    KUBECLIENT.get_or_init(|| client);
}

impl<M> KubeAPI<M>
where
    M: Resource + Metadata<Ty = ObjectMeta> + Clone + Debug + for<'a> Deserialize<'a> + Serialize,
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
        let crd = Api::<M>::all(self.client.clone());
        let pp = PostParams::default();
        let res = crd.create(&pp, &self.model).await?;
        Ok(res)
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        let crd = Api::<M>::all(self.client.clone());
        let dp = DeleteParams::default();
        crd.delete(name, &dp).await?;
        Ok(())
    }
}
