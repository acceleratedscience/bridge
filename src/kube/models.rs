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
    ) -> Self {
        NotebookSpec {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name,
                        image,
                        image_pull_policy,
                    }],
                    image_pull_secrets: ImagePullSecret {
                        name: image_pull_secret,
                    },
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
    image_pull_secrets: ImagePullSecret,
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

        let spec = NotebookSpec::new(name, image, image_pull_policy, image_pull_secret);

        let expected = json!({
            "template": {
                "spec": {
                    "containers": [
                        {
                            "name": "notebook",
                            "image": "gcr.io/kubeflow-images-public/tensorflow-2.1.0-notebook-gpu:1.0.0",
                            "imagePullPolicy": "Always"
                        }
                    ],
                    "imagePullSecrets": {
                        "name": "gcr-secret"
                    }
                }
            }
        });

        let actual = serde_json::to_value(&spec).unwrap();
        assert_eq!(actual, expected);
    }
}
