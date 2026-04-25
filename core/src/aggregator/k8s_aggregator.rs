use moka::sync::Cache;
use std::sync::{Arc, RwLock};

use crate::topology::Topology;

#[derive(Clone)]
pub struct PodInfo {
    pub namespace: String,
    pub service_name: String,
    pub service_type: String,
}

#[derive(Clone)]
pub struct ServiceInfo {
    pub namespace: String,
    pub service_type: String,
    pub cluster_ip: String,
}

pub struct K8sAggregator {
    pod_cache: Arc<Cache<String, PodInfo>>,
    service_cache: Arc<Cache<String, ServiceInfo>>,
    topology: Arc<RwLock<Topology>>,
}

impl K8sAggregator {
    pub fn new(topology: Arc<RwLock<Topology>>) -> Self {
        Self {
            pod_cache: Arc::new(Cache::builder().build()),
            service_cache: Arc::new(Cache::builder().build()),
            topology,
        }
    }

    pub fn pod_cache(&self) -> Arc<Cache<String, PodInfo>> {
        self.pod_cache.clone()
    }

    pub fn record_pod_applied(&self, pod_ip: &str, namespace: &str, service_name: &str, service_type: &str) {
        self.pod_cache.insert(pod_ip.to_string(), PodInfo {
            namespace: namespace.to_string(),
            service_name: service_name.to_string(),
            service_type: service_type.to_string(),
        });
        // If we now know the real service name, fix any stale topology nodes
        // that were created with the raw IP before this mapping existed.
        if !service_name.is_empty() {
            if let Ok(mut topo) = self.topology.write() {
                topo.resolve_unknown_node(pod_ip, service_name, namespace);
            } else {
                log::warn!("Failed to acquire topology write lock — skipping resolve for {}", pod_ip);
            }
        }

        log::info!("Pod cache ({} entries):", self.pod_cache.entry_count());
        for (ip, info) in self.pod_cache.iter() {
            log::info!("  {} -> {}/{}", ip, info.namespace, info.service_name);
        }
    }

    pub fn record_pod_deleted(&self, pod_ip: &str) {
        self.pod_cache.remove(pod_ip);
    }

    pub fn record_service_applied(&self, name: &str, namespace: &str, service_type: &str, cluster_ip: &str) {
        let key = format!("{}/{}", namespace, name);
        self.service_cache.insert(key, ServiceInfo {
            namespace: namespace.to_string(),
            service_type: service_type.to_string(),
            cluster_ip: cluster_ip.to_string(),
        });
    }

    pub fn record_service_deleted(&self, name: &str, namespace: &str) {
        let key = format!("{}/{}", namespace, name);
        self.service_cache.remove(&key);
    }
}
