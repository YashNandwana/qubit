use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client, runtime::watcher::{self, Event}};

use super::generic_handler::GenericHandler;
use super::informer::{EventHandler, Informer};

// Type alias to disambiguate GenericHandler's blanket EventHandler impl.
// The compiler needs to know which K to use when calling on_init_done().
type NodeHandler = dyn EventHandler<Node>;

/// Standalone informer for the cluster-scoped Node resource.
///
/// InformerGeneric constrains `K: Resource<Scope = NamespaceResourceScope>`,
/// but Node has `ClusterResourceScope`. Rather than complicating the generic
/// informer's type bounds for a single resource, this struct implements
/// `Informer` directly. The pattern is identical — just uses
/// `Api::<Node>::all(client)` which is the cluster-scoped API constructor.
pub struct NodeInformer {
    handler: Arc<GenericHandler>,
}

impl NodeInformer {
    pub fn new(handler: GenericHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
}

#[async_trait]
impl Informer for NodeInformer {
    async fn start(&self, client: Client) -> Result<(), String> {
        let api: Api<Node> = Api::all(client);

        log::info!("Starting Node informer (cluster-scoped)");

        let mut watcher =
            watcher::watcher(api, watcher::Config::default()).boxed();

        // Cast to NodeHandler so the compiler knows which EventHandler<K> impl to use.
        // GenericHandler has a blanket impl for all K, so without this the compiler
        // can't pick a concrete K for methods like on_init_done().
        let handler: &NodeHandler = self.handler.as_ref();

        while let Some(event) = watcher.next().await {
            match event {
                Ok(Event::Apply(obj)) => handler.on_apply(&obj),
                Ok(Event::Delete(obj)) => handler.on_delete(&obj),
                Ok(Event::Init) => {}
                Ok(Event::InitApply(obj)) => handler.on_init_apply(&obj),
                Ok(Event::InitDone) => handler.on_init_done(),
                Err(e) => return Err(format!("Node watcher error: {}", e)),
            }
        }
        Ok(())
    }
}
