#![allow(dead_code)]

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const OWUI: &str = "owui";

/*
apiVersion: accelerate.science/v1
kind: Owui
metadata:
  name: u67aceff11a66c1fa7c99726c-openwebui
  namespace: openwebui
spec:
  # will remove this... makes no sense to have this
  replica: 1
  retainPVC: false

  servicePort: 8080

  image:
    registry: quay.io
    repository: ibmdpdev/open-webui-spati
    tag: latest
    pullPolicy: Always

  persistence:
    size: 2Gi
    storageClass: gp3

  env:
    - name: MOLVIEWER_URL
      value: "moleviewer.open.accelerate.science"
    - name: WEBUI_SECRET_KEY
      # i dunno just make one up
      value: "idontcarewhatthisisbecuasethisisrequiredforsomereason"
 */

// This is a placeholder for openwebui CRD
#[derive(CustomResource, Clone, Deserialize, Serialize, Debug, JsonSchema)]
#[kube(
    group = "accelerate.science",
    version = "v1",
    kind = "Owui",
    namespaced
)]
struct OpenWebUI {
    replica: u8,
    #[serde(rename = "retainPVC")]
    retain_pvc: bool,
    #[serde(rename = "servicePort")]
    service_port: u16,
    image: Image,
    env: Vec<Env>,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
struct Image {
    registry: String,
    repository: String,
    tag: String,
    #[serde(rename = "pullPolicy")]
    pull_policy: String,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
struct Persistence {
    size: String,
    #[serde(rename = "storageClass")]
    storage_class: String,
}

#[derive(Clone, Deserialize, Serialize, Debug, JsonSchema)]
struct Env {
    name: String,
    value: String,
}

#[cfg(test)]
mod test {
    use crate::kube::KubeAPI;

    #[test]
    fn test_deserialize_owui() {
        // let yaml_data = r#"
        //     apiVersion: accelerate.science/v1
        //     kind: Owui
        //     metadata:
        //       name: u67aceff11a66c1fa7c99726c-openwebui
        //       namespace: openwebui
        //     spec:
        //       # will remove this... makes no sense to have this
        //       replica: 1
        //       retainPVC: false
        //
        //       servicePort: 8080
        //
        //       image:
        //         registry: quay.io
        //         repository: ibmdpdev/open-webui-spati
        //         tag: latest
        //         pullPolicy: Always
        //
        //       persistence:
        //         size: 2Gi
        //         storageClass: gp3
        //
        //       env:
        //         - name: MOLVIEWER_URL
        //           value: "moleviewer.open.accelerate.science"
        //         - name: WEBUI_SECRET_KEY
        //           value: "idontcarewhatthisisbecuasethisisrequiredforsomereason"
        // "#;
        // match serde_json::from_str::<super::OpenWebUI>(yaml_data) {
        //     Ok(owui) => println!("Successfully deserialized OpenWebUI: {:?}", owui),
        //     Err(e) => eprintln!("Failed to deserialize OpenWebUI: {}", e),
        // }

        let owui = super::Owui {
            spec: super::OpenWebUI {
                replica: 1,
                retain_pvc: false,
                service_port: 8080,
                image: super::Image {
                    registry: "quay.io".to_string(),
                    repository: "ibmdpdev/open-webui-spati".to_string(),
                    tag: "latest".to_string(),
                    pull_policy: "Always".to_string(),
                },
                env: vec![
                    super::Env {
                        name: "MOLVIEWER_URL".to_string(),
                        value: "moleviewer.open.accelerate.science".to_string(),
                    },
                    super::Env {
                        name: "WEBUI_SECRET_KEY".to_string(),
                        value: "idontcarewhatthisisbecuasethisisrequiredforsomereason".to_string(),
                    },
                ],
            },
            metadata: kube::api::ObjectMeta {
                name: Some("u67aceff11a66c1fa7c99726c-openwebui".to_string()),
                namespace: Some("openwebui".to_string()),
                ..Default::default()
            },
        };
        // print as json
        match serde_json::to_string_pretty(&owui) {
            Ok(json) => println!("Successfully serialized OpenWebUI to JSON:\n{}", json),
            Err(e) => eprintln!("Failed to serialize OpenWebUI to JSON: {}", e),
        }

        // KubeAPI::new(owui);
    }
}
