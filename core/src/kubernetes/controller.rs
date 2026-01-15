use std::sync::Arc;
use crate::config::QubitConfig;

use kube;
use kube::{config, Client, Config};
use kube::runtime::watcher::{Event, Config as WatcherConfig};
use kube::runtime::watcher;
use kube::api::Api;

use crate::kubernetes::informer::{informer_service_factory, Informer, InformerType};
use crate::kubernetes::informer::InformerType::{ConfigMap, Service};

use futures::StreamExt;
use tokio::task::JoinHandle;
use log;

pub struct Controller {
    config:                 Arc<QubitConfig>,
    namespace:              Option<String>,
    service_informer:       Arc<dyn Informer + Send + Sync>,
    config_map_informer:    Arc<dyn Informer + Send + Sync>,
}

impl Controller {
    pub fn new(config: Arc<QubitConfig>, namespace: Option<String>) -> Self {
        let service_informer = Self::get_informer_arc(config.clone(), Service);
        let config_map_informer = Self::get_informer_arc(config.clone(), ConfigMap);
        
        Self {
            config,
            namespace,
            service_informer,
            config_map_informer
        }
    }

    pub async fn start_informers(&self) -> Result<(), String> {
        let kube_cfg = match Config::incluster() {
            Ok(cfg) => cfg,
            Err(_) => {
                Config::infer()
                    .await
                    .map_err(|e| format!("failed to infer kube config: {}", e))?
            }
        };

        let client = match Client::try_from(kube_cfg) {
            Ok(client) => client,
            Err(e) => {
                return Err(format!("failed to fetch kube client{}", e))
            }
        };

        let ns = self.namespace.clone().unwrap_or_default();
        let configmap_api: Api<k8s_openapi::api::core::v1::ConfigMap> = if ns.is_empty() {
            Api::all(client.clone())
        } else {
            Api::namespaced(client.clone(), &ns)
        };

        let service_api: Api<k8s_openapi::api::core::v1::Service> = if ns.is_empty() {
            Api::all(client.clone())
        } else {
            Api::namespaced(client.clone(), &ns)
        };

        let mut watcher_cfg = watcher::Config::default();
        let service_informer = Arc::clone(&self.service_informer);
        let configmap_informer = Arc::clone(&self.config_map_informer);

        let cm_handle: JoinHandle<Result<(), String>> = tokio::spawn(async move {
            let mut cm_watcher = watcher(configmap_api, watcher_cfg.clone()).boxed();

            // next().await -> Option<Result<Event<ConfigMap>, watcher::Error>>
            while let Some(event_result) = cm_watcher.next().await {
                match event_result {
                    Ok(Event::Apply(obj)) => {
                        configmap_informer.event_handler();
                        configmap_informer.process_add_event();
                    }
                    Ok(Event::Delete(obj)) => {
                        configmap_informer.event_handler();
                        configmap_informer.process_delete_event();
                    }
                    Err(e) => {
                        return Err(format!("configmap watcher error: {}", e));
                    }
                    _ => {}
                }
            }
            Ok(())
        });

        watcher_cfg = watcher::Config::default();
        let svc_handle: JoinHandle<Result<(), String>> = tokio::spawn(async move {
            let mut svc_watcher = watcher(service_api, watcher_cfg.clone()).boxed();
            while let Some(event) = svc_watcher.next().await {
                match event {
                    Ok(Event::Apply(obj)) => {
                        service_informer.event_handler();
                        service_informer.process_add_event();
                    }
                    Ok(Event::Delete(_obj)) => {
                        service_informer.event_handler();
                        service_informer.process_delete_event();
                    }
                    Err(e) => {
                        return Err(format!("service watcher error: {}", e))
                    },
                    _ => {}
                }
            }
            Ok(())
        });

        futures::future::pending::<()>().await;
        log::info!("Started all k8s informers");
        Ok(())
    }

    fn get_informer_arc(cfg: Arc<QubitConfig>, resource: InformerType) -> Arc<dyn Informer + Send + Sync> {
        match resource {
            InformerType::Service => {
                let service_box = informer_service_factory(cfg.clone(),resource);
                let service_informer: Arc<dyn Informer + Send + Sync> = Arc::from(service_box);
                service_informer
            },
            InformerType::ConfigMap => {
                let config_map_box = informer_service_factory(cfg.clone(),resource);
                let config_map_informer: Arc<dyn Informer + Send + Sync> = Arc::from(config_map_box);
                config_map_informer
            }
        }
    }
}
