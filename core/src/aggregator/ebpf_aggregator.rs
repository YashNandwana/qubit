use moka::sync::Cache;
use std::sync::{Arc, RwLock};

use crate::aggregator::k8s_aggregator::PodInfo;
use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::model::{EbpfNetworkEvent, Error};
use crate::topology::{Flow, NodeId, Topology};

pub struct EbpfAggregator {
    config: Arc<QubitConfig>,
    db: Arc<DAO>,
    topology: Arc<RwLock<Topology>>,
    pod_cache: Arc<Cache<String, PodInfo>>,
}

impl EbpfAggregator {
    pub fn new(
        config: Arc<QubitConfig>,
        db: Arc<DAO>,
        topology: Arc<RwLock<Topology>>,
        pod_cache: Arc<Cache<String, PodInfo>>,
    ) -> Self {
        Self { config, db, topology, pod_cache }
    }

    pub async fn record_ebpf_event(&self, event: EbpfNetworkEvent) -> Result<String, Error> {
        log::info!("recorded ebpf event: {}", event);

        self.db
            .add_event(event.clone())
            .await
            .map_err(|e| Error::EbpfEventRecordingFailed(e.to_string()))?;

        let pod_info = self.pod_cache.get(&event.src_ip);
        let src_namespace = pod_info.as_ref().map(|p| p.namespace.as_str()).unwrap_or("unknown");
        let src_service = pod_info.as_ref()
            .filter(|p| !p.service_name.is_empty())
            .map(|p| p.service_name.as_str())
            .unwrap_or(&event.src_ip);

        let source = NodeId {
            service_name: src_service.to_string(),
            namespace: src_namespace.to_string(),
        };

        let (dst_service, dst_namespace) = parse_k8s_host(&event.host);
        let destination = NodeId {
            service_name: dst_service,
            namespace: dst_namespace,
        };

        let flow = Flow {
            source_node: source,
            destination_node: destination,
            path: event.path,
            method: event.method,
        };

        self.topology
            .write()
            .map_err(|_| Error::EbpfEventRecordingFailed("topology lock poisoned".to_string()))?
            .add_flow(flow);

        Ok("saved event!".to_string())
    }
}

/// Parses a Kubernetes host header into (service_name, namespace).
/// "service-b.default.svc.cluster.local" -> ("service-b", "default")
/// "service-b.default"                   -> ("service-b", "default")
/// "service-b"                           -> ("service-b", "unknown")
fn parse_k8s_host(host: &str) -> (String, String) {
    let parts: Vec<&str> = host.splitn(3, '.').collect();
    match parts.as_slice() {
        [svc, ns, ..] => (svc.to_string(), ns.to_string()),
        [svc] => (svc.to_string(), "unknown".to_string()),
        _ => (host.to_string(), "unknown".to_string()),
    }
}
