use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use futures::StreamExt;
use kube::api::Resource;
use kube::core::NamespaceResourceScope;
use kube::{
    runtime::watcher::{self, Event},
    Api, Client,
};
use serde::de::DeserializeOwned;

use crate::config::ClusterAgentConfig;

#[async_trait]
pub trait Informer: Send + Sync {
    async fn start(&self, client: Client) -> Result<(), String>;
}

pub trait EventHandler<K: Debug>: Send + Sync {
    fn on_apply(&self, _resource: &K) {}
    fn on_delete(&self, _resource: &K) {}
    fn on_init_apply(&self, _resource: &K) {}
    fn on_init_done(&self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformerType {
    Service,
    ConfigMap,
    Pod,
    Deployment,
    ReplicaSet,
    Ingress,
    Event,
    Hpa,
    Node,
    Rollout,
    ExternalSecret,
    HttpProxy,
    VirtualService,
}

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
    config: Arc<ClusterAgentConfig>,
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
    pub fn new(
        config: Arc<ClusterAgentConfig>,
        informer_type: InformerType,
        handler: H,
    ) -> Arc<Self> {
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
                Ok(Event::Apply(obj)) => self.handler.on_apply(&obj),
                Ok(Event::Delete(obj)) => self.handler.on_delete(&obj),
                Ok(Event::Init) => {}
                Ok(Event::InitApply(obj)) => self.handler.on_init_apply(&obj),
                Ok(Event::InitDone) => self.handler.on_init_done(),
                Err(e) => return Err(format!("{} watcher error: {}", type_name, e)),
            }
        }
        Ok(())
    }
}
