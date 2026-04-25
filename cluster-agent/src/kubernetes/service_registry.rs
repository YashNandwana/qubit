use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;

use anyhow::{Error};
use k8s_openapi::api::core::v1::{Pod, Service};

struct RegisteredService {
    name: String,
    service_type: String,
    // k8s-openapi uses BTreeMap for label maps, not HashMap
    selector: BTreeMap<String, String>,
}

/// Local cache of service selectors, maintained by ServiceHandler.
/// PodHandler reads from it to enrich pod events with service info.
pub struct ServiceRegistry {
    // key: "namespace/name"
    services: RwLock<HashMap<String, RegisteredService>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, svc: &Service) -> Result<String, Error> {
        let name = match svc.metadata.name.as_deref() {
            Some(n) => n.to_string(),
            None => return Ok("no service found!".to_string()),
        };
        let namespace = svc.metadata.namespace.clone().unwrap_or_default();
        let spec = match svc.spec.as_ref() {
            Some(s) => s,
            None => return Ok("No spec found!".to_string()),
        };
        let selector = spec.selector.clone().unwrap_or_default();
        if selector.is_empty() {
            // Services without selectors (e.g. ExternalName) don't manage pods
            return Ok("No selector found on service".to_string());
        }
        let service_type = spec.type_.clone().unwrap_or_else(|| "ClusterIP".to_string());
        let key = format!("{}/{}", namespace, name);
        self.services.write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?
            .insert(
            key,
            RegisteredService { name, service_type, selector },
        );
        
        Ok("regisetered services".to_string())
    }

    pub fn deregister(&self, name: &str, namespace: &str) -> Result<String, Error> {
        let key = format!("{}/{}", namespace, name);
        self.services.write().map_err(|_| anyhow::anyhow!("lock poisoned"))?.remove(&key);
        Ok("deregisterd service".to_string())
    }

    /// Returns `(service_name, service_type)` if any registered service's
    /// selector is a subset of this pod's labels.
    pub fn find_service_for_pod(&self, pod: &Pod) -> Option<(String, String)> {
        let pod_labels = pod.metadata.labels.as_ref()?;
        let guard = self.services.read().ok()?;
        for registered in guard.values() {
            if registered
                .selector
                .iter()
                .all(|(k, v)| pod_labels.get(k) == Some(v))
            {
                return Some((registered.name.clone(), registered.service_type.clone()));
            }
        }
        None
    }
}
