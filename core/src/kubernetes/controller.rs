use std::sync::Arc;

use super::informer_factory::{InformerFactory, InformerFactoryImpl};
use super::model::InformerModel;
use crate::config::QubitConfig;

pub trait AbstractController {
    fn create_informers(&self) -> InformerModel;
}
pub struct Controller {
    config: Arc<QubitConfig>,
}

impl Controller {
    pub fn new(config: Arc<QubitConfig>) -> Box<dyn AbstractController + Send + Sync + 'static> {
        Box::new(Self { config })
    }
}

impl AbstractController for Controller {
    fn create_informers(&self) -> InformerModel {
        let factory = InformerFactoryImpl::new(self.config.clone());

        InformerModel::new(
            factory.create_configmap_informer(),
            factory.create_service_informer(),
        )
    }
}
