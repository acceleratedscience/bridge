use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{PersistentVolumeClaim, VolumeResourceRequirements},
    apimachinery::pkg::api::resource::Quantity,
};
use kube::{api::ObjectMeta, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::CONFIG;

pub const NAMESPACE: &str = "notebook";

// Define the Notebook CRD struct
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "kubeflow.org", version = "v1", kind = "Notebook", namespaced)]
pub struct NotebookSpec {
    template: NotebookTemplateSpec,
}

impl NotebookSpec {
    pub fn new(
        name: String,
        image_pull_secret: String,
        command: Option<Vec<String>>,
        args: Option<Vec<String>>,
        notebook_env: Option<Vec<String>>,
        volume_name: String,
        volume_mount_path: String,
    ) -> Self {
        NotebookSpec {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name,
                        image: CONFIG.notebook_image.clone(),
                        image_pull_policy: CONFIG.notebook_image_pull_policy.clone(),
                        volume_mounts: Some(vec![VolumeMount {
                            name: volume_name.clone(),
                            mount_path: volume_mount_path,
                        }]),
                        command,
                        args,
                        env: Some(vec![EnvVar {
                            name: "NOTEBOOK_ARGS".to_string(),
                            // resorting to default here if the vec is empty, but the vec should
                            // never be empty
                            value: notebook_env.unwrap_or_default().join(" "),
                        }]),
                    }],
                    image_pull_secrets: Some(vec![ImagePullSecret {
                        name: image_pull_secret,
                    }]),
                    volumes: Some(vec![VolumeSpec {
                        name: volume_name.clone(),
                        persistent_volume_claim: Some(PersistentVolumeClaimSpec {
                            claim_name: volume_name,
                            read_only: None,
                        }),
                        config_map: None,
                    }]),
                },
            },
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct NotebookTemplateSpec {
    spec: PodSpec,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct PodSpec {
    containers: Vec<ContainerSpec>,
    #[serde(rename = "imagePullSecrets")]
    image_pull_secrets: Option<Vec<ImagePullSecret>>,
    volumes: Option<Vec<VolumeSpec>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ImagePullSecret {
    name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ContainerSpec {
    name: String,
    image: String,
    #[serde(rename = "imagePullPolicy")]
    image_pull_policy: String,
    #[serde(rename = "volumeMounts")]
    volume_mounts: Option<Vec<VolumeMount>>,
    command: Option<Vec<String>>,
    args: Option<Vec<String>>,
    env: Option<Vec<EnvVar>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct EnvVar {
    name: String,
    value: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct VolumeMount {
    name: String,
    #[serde(rename = "mountPath")]
    mount_path: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct VolumeSpec {
    name: String,
    #[serde(rename = "persistentVolumeClaim")]
    #[serde(skip_serializing_if = "Option::is_none")]
    persistent_volume_claim: Option<PersistentVolumeClaimSpec>,
    #[serde(rename = "configMap")]
    #[serde(skip_serializing_if = "Option::is_none")]
    config_map: Option<ConfigMapSpec>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ConfigMapSpec {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct PersistentVolumeClaimSpec {
    #[serde(rename = "claimName")]
    claim_name: String,
    #[serde(rename = "readOnly")]
    #[serde(skip_serializing_if = "Option::is_none")]
    read_only: Option<bool>,
}

// Define the PVC Spec
pub struct PVCSpec {
    pub spec: PersistentVolumeClaim,
}

impl PVCSpec {
    pub fn new(name: String, storage_size: usize) -> Self {
        Self {
            spec: PersistentVolumeClaim {
                metadata: ObjectMeta {
                    name: Some(name),
                    ..Default::default()
                },
                spec: Some(k8s_openapi::api::core::v1::PersistentVolumeClaimSpec {
                    access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                    resources: Some(VolumeResourceRequirements {
                        requests: Some(BTreeMap::from([(
                            "storage".to_string(),
                            Quantity(storage_size.to_string() + "Gi"),
                        )])),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_notebook_spec() {
        let name = "notebook".to_string();
        let image_pull_secret = "gcr-secret".to_string();
        let volume_name = "notebook-volume".to_string();
        let volume_mount_path = "/mnt/notebook".to_string();
        let command = Some(vec!["/bin/bash".to_string()]);
        let args = Some(vec!["-c".to_string(), "echo 'Hello, World!'".to_string()]);
        let env = Some(vec!["FOO=bar".to_string()]);

        let spec = NotebookSpec::new(
            name,
            image_pull_secret,
            command,
            args,
            env,
            volume_name,
            volume_mount_path,
        );

        let expected = json!({
            "template": {
                "spec": {
                    "containers": [
                        {
                            "name": "notebook",
                            "image": "quay.io/ibmdpdev/openad_workbench:latest",
                            "imagePullPolicy": "IfNotPresent",
                            "volumeMounts": [
                                {
                                    "name": "notebook-volume",
                                    "mountPath": "/mnt/notebook"
                                }
                            ],
                            "command": ["/bin/bash"],
                            "args": ["-c", "echo 'Hello, World!'"],
                            "env": [
                                {
                                    "name": "NOTEBOOK_ARGS",
                                    "value": "FOO=bar"
                                }
                            ]
                        }
                    ],
                    "imagePullSecrets": [
                        {
                            "name": "gcr-secret"
                        }
                    ],
                    "volumes": [
                        {
                            "name": "notebook-volume",
                            "persistentVolumeClaim": {
                                "claimName": "notebook-volume-pvc"
                            }
                        }
                    ]
                }
            }
        });

        let actual = serde_json::to_value(&spec).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_pvc_spec() {
        let name = "notebook-volume".to_string();
        let storage_size = 10;

        let spec = PVCSpec::new(name, storage_size);

        let expected = json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": {
                "name": "notebook-volume-pvc"
            },
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "resources": {
                    "requests": {
                        "storage": "10Gi"
                    }
                }
            }
        });

        let actual = serde_json::to_value(&spec.spec).unwrap();
        assert_eq!(actual, expected);
    }
}
