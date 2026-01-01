use std::sync::Arc;
use crate::config::QubitConfig;
use super::informer::{Informer, InformerType};

pub struct ServiceInformer {
    pub config: Arc<QubitConfig>,
    pub informer_type: InformerType,
}

impl ServiceInformer {
    pub fn new(
        config: Arc<QubitConfig>,
        informer_type: InformerType
    ) -> Self {
        Self {config, informer_type}
    }
}

impl Informer for ServiceInformer {
    fn start_informer(&self) {
        log::info!("starting configmap informer")
    }

    fn event_handler(&self) {
        log::info!("handling config event")
    }

    fn process_add_event(&self) {
        log::info!("handling config event for add")
    }

    fn process_update_event(&self) {
        log::info!("handling config event for update")
    }

    fn process_delete_event(&self) {
        log::info!("handling config event for delete")
    }
}