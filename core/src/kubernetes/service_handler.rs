use k8s_openapi::api::core::v1::Service;

use super::informer::EventHandler;

pub struct ServiceHandler;

impl EventHandler<Service> for ServiceHandler {
    fn on_apply(&self, svc: &Service) {
        let name = svc.metadata.name.as_deref().unwrap_or("unknown");
        log::info!("Service applied: {}", name);
    }

    fn on_delete(&self, svc: &Service) {
        let name = svc.metadata.name.as_deref().unwrap_or("unknown");
        log::info!("Service deleted: {}", name);
        // TODO: Store deletion event for tracking
    }

    fn on_init_apply(&self, svc: &Service) {
        let name = svc.metadata.name.as_deref().unwrap_or("unknown");
        log::debug!("Service discovered during init: {}", name);
    }

    fn on_init_done(&self) {
        log::info!("Service initial sync complete");
    }
}