use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use futures::StreamExt;
use kube::api::Resource;
use kube::core::NamespaceResourceScope;
use kube::{
    Api, Client,
    runtime::watcher::{self, Event},
};
use serde::de::DeserializeOwned;

use crate::config::QubitConfig;

#[async_trait]
pub trait Informer: Send + Sync {
    async fn start(&self, client: Client) -> Result<(), String>;
}

/// Trait for handling resource events with default implementations.
/// Override only the methods needed for resource-specific behavior.
pub trait EventHandler<K: Debug>: Send + Sync {
    fn on_apply(&self, _resource: &K) {
        log::info!("Resource applied: {:?}", _resource);
    }

    fn on_delete(&self, _resource: &K) {
        log::info!("Resource deleted: {:?}", _resource);
    }

    fn on_init_apply(&self, _resource: &K) {
        log::info!("Resource discovered during init: {:?}", _resource);
    }

    fn on_init_done(&self) {
        log::info!("Resource initial sync complete");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformerType {
    Service,
    ConfigMap,
}

/// Generic informer that works with any namespace-scoped Kubernetes resource type.
/// Accepts an EventHandler to customize event processing per resource type.
pub struct InformerGeneric<K, H>
where
    K: Resource<Scope = NamespaceResourceScope>
        + Clone
        + DeserializeOwned
        + Debug
        + Send
        + Sync
        + 'static,
    H: EventHandler<K>,
{
    config: Arc<QubitConfig>,
    watcher_cfg: watcher::Config,
    informer_type: InformerType,
    handler: Arc<H>,
    _marker: PhantomData<K>,
}

impl<K, H> InformerGeneric<K, H>
where
    K: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + DeserializeOwned
        + Debug
        + Send
        + Sync
        + 'static,
    H: EventHandler<K> + 'static,
{
    pub fn new(config: Arc<QubitConfig>, informer_type: InformerType, handler: H) -> Arc<Self> {
        Arc::new(Self {
            config,
            watcher_cfg: watcher::Config::default(),
            informer_type,
            handler: Arc::new(handler),
            _marker: PhantomData,
        })
    }
}

#[async_trait]
impl<K, H> Informer for InformerGeneric<K, H>
where
    K: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + DeserializeOwned
        + Debug
        + Send
        + Sync
        + 'static,
    H: EventHandler<K> + 'static,
{
    async fn start(&self, client: Client) -> Result<(), String> {
        let namespace = &self.config.kubernetes.namespace;

        let api: Api<K> = if namespace.is_empty() {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        };

        let type_name = format!("{:?}", self.informer_type);
        log::info!(
            "Starting {} informer for namespace: {}",
            type_name,
            if namespace.is_empty() {
                "all"
            } else {
                namespace
            }
        );

        let mut watcher = watcher::watcher(api, self.watcher_cfg.clone()).boxed();

        while let Some(event) = watcher.next().await {
            match event {
                Ok(Event::Apply(obj)) => {
                    log::debug!("{} applied: {:?}", type_name, obj);
                    self.handler.on_apply(&obj);
                }
                Ok(Event::Delete(obj)) => {
                    log::debug!("{} deleted: {:?}", type_name, obj);
                    self.handler.on_delete(&obj);
                }
                Ok(Event::Init) => {
                    log::debug!("{} watcher initialized", type_name);
                }
                Ok(Event::InitApply(obj)) => {
                    log::debug!("{} init apply: {:?}", type_name, obj);
                    self.handler.on_init_apply(&obj);
                }
                Ok(Event::InitDone) => {
                    log::info!("{} watcher init complete", type_name);
                    self.handler.on_init_done();
                }
                Err(e) => {
                    return Err(format!("{} watcher error: {}", type_name, e));
                }
            }
        }
        Ok(())
    }
}
