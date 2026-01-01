use std::sync::Arc;
use crate::config::QubitConfig;

pub trait Informer: Send + Sync {
    fn start_informer(&self);
    fn event_handler(&self);
    fn process_add_event(&self);
    fn process_update_event(&self);
    fn process_delete_event(&self);
}

#[derive(Clone, Copy, Debug)]
pub enum InformerType {
    Service,
    ConfigMap,
}

pub struct InformerService {
    config: Arc<QubitConfig>,
}

impl InformerService {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        Self { config }
    }
}

pub fn informer_service_factory(
    config: Arc<QubitConfig>,
    resource: InformerType,
) -> Box<dyn Informer + Send + Sync> {
    match resource {
        InformerType::Service => {
            Box::new(crate::kubernetes::service_informer::ServiceInformer::new(config, resource))
        }
        InformerType::ConfigMap => {
            Box::new(crate::kubernetes::configmap_informer::ConfigMapInformer::new(config, resource))
        }
    }
}
