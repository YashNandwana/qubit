use std::sync::Arc;

use k8s_openapi::api::core::v1::Pod;

use super::informer::EventHandler;
use crate::service::ClusterAggregator;

pub struct PodHandler {
    aggregator: Arc<ClusterAggregator>,
}

impl PodHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>) -> Self {
        Self { aggregator }
    }
}

impl EventHandler<Pod> for PodHandler {
    fn on_apply(&self, pod: &Pod) {
        if let Some((ip, namespace)) = pod_ip_and_namespace(pod) {
            let aggregator = self.aggregator.clone();
            tokio::spawn(async move {
                if let Err(e) = aggregator.send_pod_applied(ip.clone(), namespace).await {
                    log::error!("Failed to send pod applied (ip={}): {}", ip, e);
                }
            });
        }
    }

    fn on_delete(&self, pod: &Pod) {
        if let Some(ip) = pod_ip(pod) {
            let aggregator = self.aggregator.clone();
            tokio::spawn(async move {
                if let Err(e) = aggregator.send_pod_deleted(ip.clone()).await {
                    log::error!("Failed to send pod deleted (ip={}): {}", ip, e);
                }
            });
        }
    }

    fn on_init_apply(&self, pod: &Pod) {
        self.on_apply(pod);
    }

    fn on_init_done(&self) {
        log::info!("Pod initial sync complete");
    }
}

fn pod_ip_and_namespace(pod: &Pod) -> Option<(String, String)> {
    let ip = pod.status.as_ref()?.pod_ip.clone()?;
    let namespace = pod.metadata.namespace.clone().unwrap_or_default();
    Some((ip, namespace))
}

fn pod_ip(pod: &Pod) -> Option<String> {
    pod.status.as_ref()?.pod_ip.clone()
}
