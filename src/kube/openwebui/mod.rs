use std::borrow::Cow;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const OWUI: &str = "owui";

#[derive(CustomResource, Clone, Deserialize, Serialize, Debug, JsonSchema)]
#[kube(
    group = "accelerate.science",
    version = "v1",
    kind = "Owui",
    namespaced
)]
pub struct OpenWebUI {
    pub replica: u8,
    #[serde(rename = "retainPVC")]
    pub retain_pvc: bool,
    #[serde(rename = "servicePort")]
    pub service_port: u16,
    pub image: Image,
    pub persistence: Persistence,
    pub env: Vec<Env>,
    #[serde(rename = "imagePullSecrets", skip_serializing_if = "Option::is_none")]
    pub image_pull_secrets: Option<Vec<ImagePullSecret>>,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
pub struct ImagePullSecret {
    pub name: Cow<'static, str>,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
pub struct Image {
    pub registry: Cow<'static, str>,
    pub repository: Cow<'static, str>,
    pub tag: Cow<'static, str>,
    #[serde(rename = "pullPolicy")]
    pub pull_policy: Cow<'static, str>,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
pub struct Persistence {
    pub size: Cow<'static, str>,
    #[serde(rename = "storageClass")]
    pub storage_class: Cow<'static, str>,
    #[serde(rename = "restoreSnapshot")]
    pub restore_snapshot: Option<Cow<'static, str>>,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
pub struct Env {
    pub name: Cow<'static, str>,
    pub value: Cow<'static, str>,
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    // use crate::kube::KubeAPI;

    #[tokio::test]
    async fn test_deserialize_owui() {
        let owui = super::Owui {
            spec: super::OpenWebUI {
                replica: 1,
                retain_pvc: false,
                service_port: 8080,
                image_pull_secrets: Some(vec![super::ImagePullSecret {
                    name: Cow::from("regcred"),
                }]),
                image: super::Image {
                    registry: Cow::from("quay.io"),
                    repository: Cow::from("ibmdpdev/open-webui-spati"),
                    tag: Cow::from("latest"),
                    pull_policy: Cow::from("Always"),
                },
                env: vec![
                    super::Env {
                        name: Cow::from("MOLVIEWER_URL"),
                        value: Cow::from("moleviewer.open.accelerate.science"),
                    },
                    super::Env {
                        name: Cow::from("WEBUI_SECRET_KEY"),
                        value: Cow::from("idontcarewhatthisisbecuasethisisrequiredforsomereason"),
                    },
                ],
                persistence: super::Persistence {
                    size: Cow::from("2Gi"),
                    storage_class: Cow::from("gp3"),
                    restore_snapshot: None,
                },
            },
            metadata: kube::api::ObjectMeta {
                name: Some("u67aceff11a66c1fa7c99726c-openwebui".to_string()),
                namespace: Some("openwebui".to_string()),
                ..Default::default()
            },
        };
        // ensure that we can serialize the OpenWebUI struct to JSON
        match serde_json::to_string_pretty(&owui) {
            Ok(json) => println!("Successfully serialized OpenWebUI to JSON:\n{}", json),
            Err(e) => panic!("Failed to serialize OpenWebUI to JSON: {}", e),
        }
        // rustls::crypto::ring::default_provider()
        //     .install_default()
        //     .expect("Cannot install default provider with ring");

        // crate::kube::init_once().await;
        // let k = KubeAPI::new(owui);
        // k.create("openwebui").await.unwrap();
    }
}
