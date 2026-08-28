use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Clone, Deserialize, Serialize, Debug, JsonSchema)]
#[kube(
    group = "snapshot.storage.k8s.io",
    version = "v1",
    kind = "VolumeSnapshot",
    namespaced
)]
// We are using this to check if the CRD exist and don't care about anything else. If in the future
// if the the content of the snapshot is needed, add them here
pub struct VolumeSnapshotSpec {}
