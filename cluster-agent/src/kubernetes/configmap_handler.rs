use std::sync::Arc;

use k8s_openapi::api::core::v1::ConfigMap;

use super::informer::EventHandler;
use crate::service::ClusterAggregator;

pub struct ConfigMapHandler {
    aggregator: Arc<ClusterAggregator>,
}

impl ConfigMapHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>) -> Self {
        Self { aggregator }
    }
}

impl EventHandler<ConfigMap> for ConfigMapHandler {
    fn on_apply(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.clone().unwrap_or_default();
        let namespace = cm.metadata.namespace.clone().unwrap_or_default();
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator.send_configmap_applied(name.clone(), namespace).await {
                log::error!("Failed to send configmap applied (name={}): {}", name, e);
            }
        });
    }

    fn on_delete(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.clone().unwrap_or_default();
        let namespace = cm.metadata.namespace.clone().unwrap_or_default();
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator.send_configmap_deleted(name.clone(), namespace).await {
                log::error!("Failed to send configmap deleted (name={}): {}", name, e);
            }
        });
    }

    fn on_init_apply(&self, cm: &ConfigMap) {
        self.on_apply(cm);
    }

    fn on_init_done(&self) {
        log::info!("ConfigMap initial sync complete");
    }
}
