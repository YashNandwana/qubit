use moka::sync::Cache;
use std::sync::{Arc, RwLock};

use crate::dao::DAO;
use crate::model::K8sResourceEvent;
use crate::topology::{Topology, K8sEvent, NodeId};

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
    db: Arc<DAO>,
}

impl K8sAggregator {
    pub fn new(topology: Arc<RwLock<Topology>>, db: Arc<DAO>) -> Self {
        Self {
            pod_cache: Arc::new(Cache::builder().build()),
            service_cache: Arc::new(Cache::builder().build()),
            topology,
            db,
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

    /// Store a generic K8s resource event. Persists to ClickHouse (1-day TTL)
    /// and attaches to the in-memory topology if a matching node exists.
    pub fn record_k8s_resource_event(
        &self,
        resource_type: &str,
        name: &str,
        namespace: &str,
        event_type: &str,
        labels: &std::collections::HashMap<String, String>,
        resource_data: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Persist to ClickHouse asynchronously
        let db_event = K8sResourceEvent {
            event_time: now as u32,
            resource_type: resource_type.to_string(),
            name: name.to_string(),
            namespace: namespace.to_string(),
            event_type: event_type.to_string(),
            labels: serde_json::to_string(labels).unwrap_or_default(),
            resource_data: resource_data.to_string(),
        };

        let db = self.db.clone();
        let resource_type_owned = resource_type.to_string();
        let name_owned = name.to_string();
        tokio::spawn(async move {
            if let Err(e) = db.add_k8s_resource_event(db_event).await {
                log::error!(
                    "Failed to persist {} event (name={}): {}",
                    resource_type_owned,
                    name_owned,
                    e
                );
            }
        });

        // 2. Attach to in-memory topology if a matching node exists
        let topo_event = K8sEvent {
            timestamp: now,
            resource_name: name.to_string(),
            resource_type: resource_type.to_string(),
            event_type: event_type.to_string(),
            event_data: resource_data.to_string(),
        };

        if let Ok(mut topo) = self.topology.write() {
            let node_id = NodeId {
                service_name: name.to_string(),
                namespace: namespace.to_string(),
            };
            topo.add_k8s_event(node_id, topo_event);
        } else {
            log::warn!("Failed to acquire topology write lock for {} event", resource_type);
        }
    }
}
