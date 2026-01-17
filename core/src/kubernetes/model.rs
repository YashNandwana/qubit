use std::sync::Arc;
use super::informer::Informer;

pub struct InformerModel {
    pub configmap: Arc<dyn Informer + Send + Sync>,
    pub service: Arc<dyn Informer + Send + Sync>,
}

impl InformerModel {
    pub fn new(configmap: Arc<dyn Informer + Send + Sync>, service: Arc<dyn Informer + Send + Sync>) -> Self {
        Self {
            configmap,
            service,
        }
    }
}