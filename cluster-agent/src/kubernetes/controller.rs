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
        )
    }
}
