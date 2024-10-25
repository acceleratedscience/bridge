use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Define the Notebook CRD struct
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "kubeflow.org", version = "v1", kind = "Notebook", namespaced)]
pub struct NotebookSpec {
    template: NotebookTemplateSpec,
}

impl Default for NotebookSpec {
    fn default() -> Self {
        Self {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name: "notebook".to_string(),
                        image: "jupyter/minimal-notebook".to_string(),
                    }],
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
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ContainerSpec {
    name: String,
    image: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_notebook_spec() {
        let notebook_spec = NotebookSpec {
            template: NotebookTemplateSpec {
                spec: PodSpec {
                    containers: vec![ContainerSpec {
                        name: "notebook".to_string(),
                        image: "jupyter/minimal-notebook".to_string(),
                    }],
                },
            },
        };

        let notebook_spec_json = json!({
            "template": {
                "spec": {
                    "containers": [
                        {
                            "name": "notebook",
                            "image": "jupyter/minimal-notebook"
                        }
                    ]
                }
            }
        });

        assert_eq!(
            serde_json::to_value(&notebook_spec).unwrap(),
            notebook_spec_json
        );
    }
}
