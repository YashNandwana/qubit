use std::sync::Arc;

use super::informer::{Informer, InformerType};
use crate::config::QubitConfig;
use crate::kubernetes::configmap_informer::ConfigMapInformer;
use crate::kubernetes::service_informer::ServiceInformer;

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
        ConfigMapInformer::new(self.config.clone(),
            InformerType::ConfigMap)
    }

    fn create_service_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        ServiceInformer::new(self.config.clone(),
            InformerType::Service)
    }
}
