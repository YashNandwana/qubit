use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use kube::api::Resource;

use super::informer::EventHandler;
use crate::service::ClusterAggregator;

/// A handler that works for any K8s resource type by extracting only metadata
/// (name, namespace, labels). This avoids creating nearly-identical handler
/// files for Deployment, ReplicaSet, Ingress, HPA, Node, and CRDs.
///
/// Uses a blanket trait implementation: `impl<K: Resource> EventHandler<K>`.
/// The existing PodHandler/ServiceHandler/ConfigMapHandler remain untouched —
/// InformerGeneric's type parameter selects the handler at compile time.
pub struct GenericHandler {
    aggregator: Arc<ClusterAggregator>,
    resource_type: String,
}

impl GenericHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>, resource_type: &str) -> Self {
        Self {
            aggregator,
            resource_type: resource_type.to_string(),
        }
    }
}

/// Blanket implementation: handles any K8s resource that implements `Resource`.
/// `Resource::meta()` gives us access to ObjectMeta (name, namespace, labels)
/// regardless of the concrete resource type.
impl<K> EventHandler<K> for GenericHandler
where
    K: Resource + Debug + Send + Sync + 'static,
{
    fn on_apply(&self, obj: &K) {
        let (name, namespace, labels) = extract_metadata(obj);
        let resource_type = self.resource_type.clone();
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_k8s_resource_event(
                    resource_type.clone(),
                    name.clone(),
                    namespace,
                    crate::proto::qubit::K8sEventType::Applied,
                    labels,
                    String::new(),
                )
                .await
            {
                log::error!(
                    "Failed to send {} applied (name={}): {}",
                    resource_type,
                    name,
                    e
                );
            }
        });
    }

    fn on_delete(&self, obj: &K) {
        let (name, namespace, labels) = extract_metadata(obj);
        let resource_type = self.resource_type.clone();
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_k8s_resource_event(
                    resource_type.clone(),
                    name.clone(),
                    namespace,
                    crate::proto::qubit::K8sEventType::Deleted,
                    labels,
                    String::new(),
                )
                .await
            {
                log::error!(
                    "Failed to send {} deleted (name={}): {}",
                    resource_type,
                    name,
                    e
                );
            }
        });
    }

    fn on_init_apply(&self, obj: &K) {
        self.on_apply(obj);
    }

    fn on_init_done(&self) {
        log::info!("{} initial sync complete", self.resource_type);
    }
}

fn extract_metadata<K: Resource>(obj: &K) -> (String, String, HashMap<String, String>) {
    let meta = obj.meta();
    let name = meta.name.clone().unwrap_or_default();
    let namespace = meta.namespace.clone().unwrap_or_default();
    let labels = meta
        .labels
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    (name, namespace, labels)
}
