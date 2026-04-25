use std::sync::Arc;

use k8s_openapi::api::core::v1::{ConfigMap, Pod, Service};

use super::configmap_handler::ConfigMapHandler;
use super::informer::{Informer, InformerGeneric, InformerType};
use super::pod_handler::PodHandler;
use super::service_handler::ServiceHandler;
use super::service_registry::ServiceRegistry;
use crate::config::ClusterAgentConfig;
use crate::service::ClusterAggregator;

pub struct InformerFactory {
    config: Arc<ClusterAgentConfig>,
    aggregator: Arc<ClusterAggregator>,
    // Shared between ServiceHandler (writer) and PodHandler (reader)
    registry: Arc<ServiceRegistry>,
}

impl InformerFactory {
    pub fn new(config: Arc<ClusterAgentConfig>, aggregator: Arc<ClusterAggregator>) -> Self {
        Self {
            config,
            aggregator,
            registry: Arc::new(ServiceRegistry::new()),
        }
    }

    pub fn create_configmap_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<ConfigMap, ConfigMapHandler>::new(
            self.config.clone(),
            InformerType::ConfigMap,
            ConfigMapHandler::new(self.aggregator.clone()),
        )
    }

    pub fn create_service_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Service, ServiceHandler>::new(
            self.config.clone(),
            InformerType::Service,
            ServiceHandler::new(self.aggregator.clone(), self.registry.clone()),
        )
    }

    pub fn create_pod_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Pod, PodHandler>::new(
            self.config.clone(),
            InformerType::Pod,
            PodHandler::new(self.aggregator.clone(), self.registry.clone()),
        )
    }
}
