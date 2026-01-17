use async_trait::async_trait;
use futures::StreamExt;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Client;
use kube::api::Api;
use kube::runtime::watcher::{self, Event};
use std::sync::Arc;

use super::informer::{Informer, InformerType};
use crate::config::QubitConfig;

/// ConfigMap-specific informer implementation
pub struct ConfigMapInformer {
    pub config: Arc<QubitConfig>,
    pub watcher_cfg: watcher::Config,
    pub informer_type: InformerType,
}

impl ConfigMapInformer {
    pub fn new(config: Arc<QubitConfig>, informer_type: InformerType) -> Arc<dyn Informer + Send + Sync> {
        Arc::new(Self {
            config,
            watcher_cfg: watcher::Config::default(),
            informer_type,
        })
    }
}

#[async_trait]
impl Informer for ConfigMapInformer {
    async fn start(&self, client: Client) -> Result<(), String> {
        let namespace = &self.config.kubernetes.namespace.clone();
        let configmap_api: Api<ConfigMap> = if namespace.is_empty() {
            Api::all(client.clone())
        } else {
            Api::namespaced(client.clone(), namespace)
        };

        log::info!(
            "Starting ConfigMap informer for namespace: {}",
            if namespace.is_empty() {
                "all"
            } else {
                namespace
            }
        );

        let mut cm_watcher = watcher::watcher(configmap_api, self.watcher_cfg.clone()).boxed();

        while let Some(event_result) = cm_watcher.next().await {
            match event_result {
                Ok(Event::Apply(obj)) => {
                    self.event_handler();
                    self.process_add_event();
                    log::debug!("ConfigMap applied: {:?}", obj.metadata.name);
                }
                Ok(Event::Delete(obj)) => {
                    self.event_handler();
                    self.process_delete_event();
                    log::debug!("ConfigMap deleted: {:?}", obj.metadata.name);
                }
                Ok(Event::Init) => {
                    log::debug!("ConfigMap watcher initialized");
                }
                Ok(Event::InitApply(obj)) => {
                    log::debug!("ConfigMap init apply: {:?}", obj.metadata.name);
                }
                Ok(Event::InitDone) => {
                    log::info!("ConfigMap watcher init complete");
                }
                Err(e) => {
                    return Err(format!("ConfigMap watcher error: {}", e));
                }
            }
        }
        Ok(())
    }

    fn event_handler(&self) {
        log::info!("Handling ConfigMap event");
    }

    fn process_add_event(&self) {
        log::info!("Processing ConfigMap add event");
    }

    fn process_update_event(&self) {
        log::info!("Processing ConfigMap update event");
    }

    fn process_delete_event(&self) {
        log::info!("Processing ConfigMap delete event");
    }
}
