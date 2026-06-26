use std::sync::Arc;

use k8s_openapi::api::core::v1::Service;

use super::informer::EventHandler;
use super::service_registry::ServiceRegistry;
use crate::service::ClusterAggregator;

pub struct ServiceHandler {
    aggregator: Arc<ClusterAggregator>,
    registry: Arc<ServiceRegistry>,
}

impl ServiceHandler {
    pub fn new(aggregator: Arc<ClusterAggregator>, registry: Arc<ServiceRegistry>) -> Self {
        Self {
            aggregator,
            registry,
        }
    }
}

impl EventHandler<Service> for ServiceHandler {
    fn on_apply(&self, svc: &Service) {
        // Update local registry first so PodHandler can look up this service immediately
        self.registry.register(svc);
        if let Some((name, namespace, service_type, cluster_ip)) = extract_fields(svc) {
            let aggregator = self.aggregator.clone();
            tokio::spawn(async move {
                if let Err(e) = aggregator
                    .send_service_applied(name.clone(), namespace, service_type, cluster_ip)
                    .await
                {
                    log::error!("Failed to send service applied (name={}): {}", name, e);
                }
            });
        }
    }

    fn on_delete(&self, svc: &Service) {
        let name = svc.metadata.name.clone().unwrap_or_default();
        let namespace = svc.metadata.namespace.clone().unwrap_or_default();
        self.registry.deregister(&name, &namespace);
        let aggregator = self.aggregator.clone();
        tokio::spawn(async move {
            if let Err(e) = aggregator
                .send_service_deleted(name.clone(), namespace)
                .await
            {
                log::error!("Failed to send service deleted (name={}): {}", name, e);
            }
        });
    }

    fn on_init_apply(&self, svc: &Service) {
        self.on_apply(svc);
    }

    fn on_init_done(&self) {
        log::info!("Service initial sync complete");
    }
}

fn extract_fields(svc: &Service) -> Option<(String, String, String, String)> {
    let name = svc.metadata.name.clone()?;
    let namespace = svc.metadata.namespace.clone().unwrap_or_default();
    let spec = svc.spec.as_ref()?;
    let service_type = spec
        .type_
        .clone()
        .unwrap_or_else(|| "ClusterIP".to_string());
    let cluster_ip = spec.cluster_ip.clone().unwrap_or_default();
    Some((name, namespace, service_type, cluster_ip))
}
