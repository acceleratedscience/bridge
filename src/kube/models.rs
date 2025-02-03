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
        notebook_image_name: &str,
        volume_name: String,
        notebook_start_url: &mut Option<String>,
        max_idle_time: &mut Option<u64>,
        env_to_add: Vec<(String, String)>,
    ) -> Self {
        let notebook_image = CONFIG.notebooks.get(notebook_image_name).unwrap();
        let mut notebook_env = notebook_image.env.clone().unwrap_or_default();

        notebook_env.push(format!(
            "--ServerApp.base_url='notebook/{}/{}'",
            NAMESPACE, name
        ));

        let mut env = vec![EnvVar {
            name: "NOTEBOOK_ARGS".to_string(),
            value: notebook_env.join(" "),
        }];

        if !env_to_add.is_empty() {
            env_to_add.into_iter().for_each(|(name, value)| {
                env.push(EnvVar { name, value });
            });
        }

        *notebook_start_url = notebook_image.start_up_url.clone();
        *max_idle_time = notebook_image.max_idle_time;

        NotebookSpec {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name,
                        image: notebook_image.url.clone(),
                        resources: Some(ResourceRequirements {
                            requests: BTreeMap::from([
                                ("cpu".to_string(), "2".to_string()),
                                ("memory".to_string(), "4Gi".to_string()),
                            ]),
                            limits: BTreeMap::from([
                                ("cpu".to_string(), "2".to_string()),
                                ("memory".to_string(), "4Gi".to_string()),
                            ]),
                        }),
                        image_pull_policy: notebook_image.pull_policy.clone(),
                        volume_mounts: Some(vec![VolumeMount {
                            name: volume_name.clone(),
                            mount_path: notebook_image.volume_mnt_path.clone().unwrap_or_default(),
                        }]),
                        command: notebook_image.command.clone(),
                        args: notebook_image.args.clone(),
                        workingdir: notebook_image.working_dir.clone(),
                        env: Some(env),
                    }],
                    image_pull_secrets: notebook_image
                        .secret
                        .clone()
                        .map(|secret| vec![ImagePullSecret { name: secret }]),
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
    resources: Option<ResourceRequirements>,
    #[serde(rename = "workingDir")]
    workingdir: Option<String>,
    #[serde(rename = "imagePullPolicy")]
    image_pull_policy: String,
    #[serde(rename = "volumeMounts")]
    volume_mounts: Option<Vec<VolumeMount>>,
    command: Option<Vec<String>>,
    args: Option<Vec<String>>,
    env: Option<Vec<EnvVar>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
struct ResourceRequirements {
    requests: BTreeMap<String, String>,
    limits: BTreeMap<String, String>,
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
        let volume_name = "notebook-volume".to_string();
        let mut start_url = None;
        let mut max_idle_time = None;

        let spec = NotebookSpec::new(
            name,
            "open_ad_workbench",
            volume_name,
            &mut start_url,
            &mut max_idle_time,
            vec![],
        );

        let expected = json!({
            "template": {
                "spec": {
                    "containers": [{
                            "name": "notebook",
                            "image": "quay.io/ibmdpdev/openad_workbench_stage:latest",
                            "resources": {
                                "requests": {
                                    "cpu": "2",
                                    "memory": "4Gi"
                                },
                                "limits": {
                                    "cpu": "2",
                                    "memory": "4Gi"
                                }
                            },
                            "workingDir": "/opt/app-root/src",
                            "imagePullPolicy": "Always",
                            "volumeMounts": [
                                {
                                    "name": "notebook-volume",
                                    "mountPath": "/opt/app-root/src"
                                }
                            ],
                            "command": null,
                            "args": null,
                            "env": [
                                {
                                    "name": "NOTEBOOK_ARGS",
                                    "value": "--ServerApp.token='' --ServerApp.password='' --ServerApp.notebook_dir='/opt/app-root/src' --ServerApp.quit_button=False --ServerApp.default_url='/lab/tree/start_menu.ipynb' --ServerApp.trust_xheaders=True --ServerApp.base_url='notebook/notebook/notebook'",
                                }]}],
                    "imagePullSecrets": [{
                        "name": "ibmdpdev-openad-pull-secret"
                    }],
                    "volumes": [{
                        "name": "notebook-volume",
                        "persistentVolumeClaim": {
                            "claimName": "notebook-volume"
                        }
                    }]
                },
            }
        });

        let actual = serde_json::to_value(&spec).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(start_url, Some("lab/tree/start_menu.ipynb".to_string()));
        assert_eq!(max_idle_time, Some(86400));
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
                "name": "notebook-volume"
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
