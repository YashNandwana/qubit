use moka::sync::Cache;
use std::sync::Arc;

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
}

impl K8sAggregator {
    pub fn new() -> Self {
        Self {
            pod_cache: Arc::new(Cache::builder().build()),
            service_cache: Arc::new(Cache::builder().build()),
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
