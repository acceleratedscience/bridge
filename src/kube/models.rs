use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Define the Notebook CRD struct
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "kubeflow.org", version = "v1", kind = "Notebook", namespaced)]
pub struct NotebookSpec {
    template: NotebookTemplateSpec,
}

impl NotebookSpec {
    pub fn new(
        name: String,
        image: String,
        image_pull_policy: String,
        image_pull_secret: String,
        command: Option<Vec<String>>,
        args: Option<Vec<String>>,
        volume_name: String,
        volume_mount_path: String,
    ) -> Self {
        NotebookSpec {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name,
                        image,
                        image_pull_policy,
                        volume_mounts: Some(vec![VolumeMount {
                            name: volume_name.clone(),
                            mount_path: volume_mount_path,
                        }]),
                        command,
                        args,
                    }],
                    image_pull_secrets: Some(vec![ImagePullSecret {
                        name: image_pull_secret,
                    }]),
                    volumes: Some(vec![VolumeSpec {
                        name: volume_name.clone(),
                        persistent_volume_claim: Some(PersistentVolumeClaimSpec {
                            claim_name: volume_name + "-pvc",
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
    read_only: Option<bool>,
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_notebook_spec() {
        let name = "notebook".to_string();
        let image = "gcr.io/kubeflow-images-public/tensorflow-2.1.0-notebook-gpu:1.0.0".to_string();
        let image_pull_policy = "Always".to_string();
        let image_pull_secret = "gcr-secret".to_string();
        let volume_name = "notebook-volume".to_string();
        let volume_mount_path = "/mnt/notebook".to_string();
        let command = Some(vec!["/bin/bash".to_string()]);
        let args = Some(vec!["-c".to_string(), "echo 'Hello, World!'".to_string()]);

        let spec = NotebookSpec::new(
            name,
            image,
            image_pull_policy,
            image_pull_secret,
            command,
            args,
            volume_name,
            volume_mount_path,
        );

        let expected = json!({
            "template": {
                "spec": {
                    "containers": [
                        {
                            "name": "notebook",
                            "image": "gcr.io/kubeflow-images-public/tensorflow-2.1.0-notebook-gpu:1.0.0",
                            "imagePullPolicy": "Always",
                            "volumeMounts": [
                                {
                                    "name": "notebook-volume",
                                    "mountPath": "/mnt/notebook"
                                }
                            ],
                            "command": ["/bin/bash"],
                            "args": ["-c", "echo 'Hello, World!'"]
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
                                "claimName": "notebook-volume-pvc",
                                "readOnly": null
                            },
                        }
                    ]
                }
            }
        });

        let actual = serde_json::to_value(&spec).unwrap();
        assert_eq!(actual, expected);
    }
}
