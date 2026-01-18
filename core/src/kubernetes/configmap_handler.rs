use k8s_openapi::api::core::v1::{ConfigMap};

use super::informer::EventHandler;

pub struct ConfigMapHandler;

impl EventHandler<ConfigMap> for ConfigMapHandler {
    fn on_apply(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.as_deref().unwrap_or("unknown");
        log::info!("ConfigMap applied: {}", name);
    }

    fn on_delete(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.as_deref().unwrap_or("unknown");
        log::info!("ConfigMap deleted: {}", name);
        // TODO: Store deletion event for tracking
    }

    fn on_init_apply(&self, cm: &ConfigMap) {
        let name = cm.metadata.name.as_deref().unwrap_or("unknown");
        log::debug!("ConfigMap discovered during init: {}", name);
    }

    fn on_init_done(&self) {
        log::info!("ConfigMap initial sync complete");
    }
}
