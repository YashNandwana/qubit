//! Custom Resource Definitions (CRDs) for optional infrastructure components.
//!
//! These use `kube::CustomResource` derive to generate types that implement
//! `Resource<Scope = NamespaceResourceScope>`, allowing them to work with
//! the existing `InformerGeneric + GenericHandler` pattern.
//!
//! The spec structs are intentionally empty — we only care about metadata
//! (name, namespace, labels). serde will silently ignore unknown fields
//! in the actual CRD spec since `deny_unknown_fields` is not set.
//!
//! If a CRD is not installed in the cluster, the watcher will error on
//! startup and the informer task exits gracefully (logged, doesn't crash
//! other informers).

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "argoproj.io",
    version = "v1alpha1",
    kind = "Rollout",
    namespaced
)]
pub struct RolloutSpec {}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "external-secrets.io",
    version = "v1beta1",
    kind = "ExternalSecret",
    namespaced
)]
pub struct ExternalSecretSpec {}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "projectcontour.io",
    version = "v1",
    kind = "HTTPProxy",
    namespaced
)]
#[kube(plural = "httpproxies")]
pub struct HttpProxySpec {}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1",
    kind = "VirtualService",
    namespaced
)]
pub struct VirtualServiceSpec {}
