use std::sync::Arc;

use k8s_openapi::api::core::v1::Pod;

use super::informer::EventHandler;
use super::service_registry::ServiceRegistry;
use crate::service::ClusterAggregator;

pub struct PodHandler {
    aggregator: Arc<ClusterAggregator>,
    registry: Arc<ServiceRegistry>,
}

impl PodHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>, registry: Arc<ServiceRegistry>) -> Self {
        Self {
            aggregator,
            registry,
        }
    }
}

impl EventHandler<Pod> for PodHandler {
    fn on_apply(&self, pod: &Pod) {
        if let Some((ip, namespace)) = pod_ip_and_namespace(pod) {
            // Try to find which service this pod belongs to via label matching
            let (service_name, service_type) =
                self.registry.find_service_for_pod(pod).unwrap_or_default();

            let application_name = resolve_application_name(pod, &service_name);

            let aggregator = self.aggregator.clone();
            tokio::spawn(async move {
                if let Err(e) = aggregator
                    .send_pod_applied(
                        ip.clone(),
                        namespace,
                        application_name,
                        service_name,
                        service_type,
                    )
                    .await
                {
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

    fn on_init_apply(&self, _pod: &Pod) {
        // Initial pod state is handled by send_initial_pod_service_map at startup.
        // Skipping here avoids overwriting correct service info with potentially
        // incomplete info if the service registry isn't fully populated yet.
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

fn resolve_application_name(pod: &Pod, service_name: &str) -> String {
    if !service_name.is_empty() {
        return service_name.to_string();
    }

    let labels = pod.metadata.labels.as_ref();
    if let Some(name) = labels.and_then(|l| l.get("app.kubernetes.io/name")) {
        return name.clone();
    }
    if let Some(name) = labels.and_then(|l| l.get("app.k8s.io/name")) {
        return name.clone();
    }
    if let Some(name) = labels.and_then(|l| l.get("app")) {
        return name.clone();
    }
    pod.metadata.name.clone().unwrap_or_default()
}
