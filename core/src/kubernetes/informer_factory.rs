use std::sync::Arc;

use k8s_openapi::api::core::v1::{ConfigMap, Service};

use super::configmap_handler::{ConfigMapHandler};
use super::service_handler::{ServiceHandler};
use super::informer::{Informer, InformerGeneric, InformerType};
use crate::config::QubitConfig;

pub trait InformerFactory {
    fn create_configmap_informer(&self) -> Arc<dyn Informer + Send + Sync>;
    fn create_service_informer(&self) -> Arc<dyn Informer + Send + Sync>;
}

pub struct InformerFactoryImpl {
    config: Arc<QubitConfig>,
}

impl InformerFactoryImpl {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        Self { config }
    }
}

impl InformerFactory for InformerFactoryImpl {
    fn create_configmap_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<ConfigMap, ConfigMapHandler>::new(
            self.config.clone(),
            InformerType::ConfigMap,
            ConfigMapHandler,
        )
    }

    fn create_service_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Service, ServiceHandler>::new(
            self.config.clone(),
            InformerType::Service,
            ServiceHandler,
        )
    }
}
