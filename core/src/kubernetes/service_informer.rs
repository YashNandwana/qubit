use async_trait::async_trait;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::Client;
use kube::api::Api;
use kube::runtime::watcher::{self, Event};
use std::sync::Arc;

use super::informer::{Informer, InformerType};
use crate::config::QubitConfig;

pub struct ServiceInformer {
    pub config: Arc<QubitConfig>,
    pub watcher_cfg: watcher::Config,
    pub informer_type: InformerType,
}

impl ServiceInformer {
    pub fn new(config: Arc<QubitConfig>, informer_type: InformerType) -> Arc<dyn Informer + Send + Sync> {
        Arc::new(Self {
            config,
            watcher_cfg: watcher::Config::default(),
            informer_type,
        })
    }
}

#[async_trait]
impl Informer for ServiceInformer {
    async fn start(&self, client: Client) -> Result<(), String> {
        let namespace = &self.config.kubernetes.namespace;
        let service_api: Api<Service> = if namespace.is_empty() {
            Api::all(client.clone())
        } else {
            Api::namespaced(client.clone(), namespace)
        };

        log::info!(
            "Starting Service informer for namespace: {}",
            if namespace.is_empty() {
                "all"
            } else {
                namespace
            }
        );

        let mut svc_watcher = watcher::watcher(service_api, self.watcher_cfg.clone()).boxed();

        while let Some(event_result) = svc_watcher.next().await {
            match event_result {
                Ok(Event::Apply(obj)) => {
                    self.event_handler();
                    self.process_add_event();
                    log::debug!("Service applied: {:?}", obj.metadata.name);
                }
                Ok(Event::Delete(obj)) => {
                    self.event_handler();
                    self.process_delete_event();
                    log::debug!("Service deleted: {:?}", obj.metadata.name);
                }
                Ok(Event::Init) => {
                    log::debug!("Service watcher initialized");
                }
                Ok(Event::InitApply(obj)) => {
                    log::debug!("Service init apply: {:?}", obj.metadata.name);
                }
                Ok(Event::InitDone) => {
                    log::info!("Service watcher init complete");
                }
                Err(e) => {
                    return Err(format!("Service watcher error: {}", e));
                }
            }
        }
        Ok(())
    }

    fn event_handler(&self) {
        log::info!("Handling Service event");
    }

    fn process_add_event(&self) {
        log::info!("Processing Service add event");
    }

    fn process_update_event(&self) {
        log::info!("Processing Service update event");
    }

    fn process_delete_event(&self) {
        log::info!("Processing Service delete event");
    }
}
