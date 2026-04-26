use std::sync::Arc;

use super::informer_factory::InformerFactory;
use super::model::InformerModel;
use crate::config::ClusterAgentConfig;
use crate::service::ClusterAggregator;

pub struct Controller {
    config: Arc<ClusterAgentConfig>,
    aggregator: Arc<ClusterAggregator>,
}

impl Controller {
    pub fn new(config: Arc<ClusterAgentConfig>, aggregator: Arc<ClusterAggregator>) -> Self {
        Self { config, aggregator }
    }

    pub fn create_informers(&self) -> InformerModel {
        let factory = InformerFactory::new(self.config.clone(), self.aggregator.clone());
        InformerModel::new(
            factory.create_configmap_informer(),
            factory.create_service_informer(),
            factory.create_pod_informer(),
            factory.create_deployment_informer(),
            factory.create_replicaset_informer(),
            factory.create_ingress_informer(),
            factory.create_event_informer(),
            factory.create_hpa_informer(),
            factory.create_node_informer(),
            factory.create_rollout_informer(),
            factory.create_external_secret_informer(),
            factory.create_http_proxy_informer(),
            factory.create_virtual_service_informer(),
        )
    }
}
