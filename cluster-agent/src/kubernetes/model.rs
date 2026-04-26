use std::sync::Arc;

use super::informer::Informer;

pub struct InformerModel {
    // Existing
    pub configmap: Arc<dyn Informer + Send + Sync>,
    pub service: Arc<dyn Informer + Send + Sync>,
    pub pod: Arc<dyn Informer + Send + Sync>,
    // Native K8s resources
    pub deployment: Arc<dyn Informer + Send + Sync>,
    pub replicaset: Arc<dyn Informer + Send + Sync>,
    pub ingress: Arc<dyn Informer + Send + Sync>,
    pub event: Arc<dyn Informer + Send + Sync>,
    pub hpa: Arc<dyn Informer + Send + Sync>,
    pub node: Arc<dyn Informer + Send + Sync>,
    // CRDs
    pub rollout: Arc<dyn Informer + Send + Sync>,
    pub external_secret: Arc<dyn Informer + Send + Sync>,
    pub http_proxy: Arc<dyn Informer + Send + Sync>,
    pub virtual_service: Arc<dyn Informer + Send + Sync>,
}

impl InformerModel {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        configmap: Arc<dyn Informer + Send + Sync>,
        service: Arc<dyn Informer + Send + Sync>,
        pod: Arc<dyn Informer + Send + Sync>,
        deployment: Arc<dyn Informer + Send + Sync>,
        replicaset: Arc<dyn Informer + Send + Sync>,
        ingress: Arc<dyn Informer + Send + Sync>,
        event: Arc<dyn Informer + Send + Sync>,
        hpa: Arc<dyn Informer + Send + Sync>,
        node: Arc<dyn Informer + Send + Sync>,
        rollout: Arc<dyn Informer + Send + Sync>,
        external_secret: Arc<dyn Informer + Send + Sync>,
        http_proxy: Arc<dyn Informer + Send + Sync>,
        virtual_service: Arc<dyn Informer + Send + Sync>,
    ) -> Self {
        Self {
            configmap,
            service,
            pod,
            deployment,
            replicaset,
            ingress,
            event,
            hpa,
            node,
            rollout,
            external_secret,
            http_proxy,
            virtual_service,
        }
    }
}
