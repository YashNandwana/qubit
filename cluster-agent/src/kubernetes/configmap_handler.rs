use std::sync::Arc;

use k8s_openapi::api::core::v1::ConfigMap;

use super::envoy_parser::parse_envoy_routes;
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

        // If this ConfigMap contains an Envoy config, parse and push routes to core.
        if let Some(envoy_yaml) = cm.data.as_ref().and_then(|d| d.get("envoy.yaml")) {
            let routes = parse_envoy_routes(envoy_yaml);
            if !routes.is_empty() {
                let agg = aggregator.clone();
                let n = name.clone();
                tokio::spawn(async move {
                    if let Err(e) = agg.send_envoy_routes(routes).await {
                        log::error!("Failed to send envoy routes from ConfigMap {}: {}", n, e);
                    }
                });
            }
        }

        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_configmap_applied(name.clone(), namespace)
                .await
            {
                log::error!("Failed to send configmap applied (name={}): {}", name, e);
            }
        });
    }

    fn on_delete(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.clone().unwrap_or_default();
        let namespace = cm.metadata.namespace.clone().unwrap_or_default();
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_configmap_deleted(name.clone(), namespace)
                .await
            {
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
