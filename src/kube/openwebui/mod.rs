#![allow(dead_code)]

use std::marker::PhantomData;

use serde::Deserialize;

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
#[derive(Deserialize)]
struct OpenWebUI {
    spec: Spec,
}

#[derive(Deserialize)]
struct Spec {
    replica: u8,
    #[serde(rename = "retainPVC")]
    retain_pvc: bool,
    #[serde(rename = "servicePort")]
    service_port: u16,
    image: Image,
    env: Vec<Env>,
}

#[derive(Deserialize)]
struct Image {
    registry: String,
    repository: String,
    tag: String,
    #[serde(rename = "pullPolicy")]
    pull_policy: String,
}

#[derive(Deserialize)]
struct Persistence {
    size: String,
    #[serde(rename = "storageClass")]
    storage_class: String,
}

#[derive(Deserialize)]
struct Env {
    name: String,
    value: String,
}
