use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;

use anyhow::Error;
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
        let service_type = spec
            .type_
            .clone()
            .unwrap_or_else(|| "ClusterIP".to_string());
        let key = format!("{}/{}", namespace, name);
        self.services
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?
            .insert(
                key,
                RegisteredService {
                    name,
                    service_type,
                    selector,
                },
            );

        Ok("regisetered services".to_string())
    }

    pub fn deregister(&self, name: &str, namespace: &str) -> Result<String, Error> {
        let key = format!("{}/{}", namespace, name);
        self.services
            .write()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?
            .remove(&key);
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use k8s_openapi::api::core::v1::{Pod, Service, ServiceSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::ServiceRegistry;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_service(name: &str, namespace: &str, selector: &[(&str, &str)]) -> Service {
        // k8s_openapi types are plain structs — we can build them directly.
        // This is the same approach used in production kube-rs controllers.
        Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(
                    selector
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                ),
                type_: Some("ClusterIP".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn make_pod(labels: &[(&str, &str)]) -> Pod {
        Pod {
            metadata: ObjectMeta {
                labels: Some(
                    labels
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                ),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn exact_match() {
        // Pod labels exactly match the service selector → service found
        let registry = ServiceRegistry::new();
        let svc = make_service("my-svc", "default", &[("app", "frontend")]);
        registry.register(&svc).unwrap();

        let pod = make_pod(&[("app", "frontend")]);
        let result = registry.find_service_for_pod(&pod);

        assert!(result.is_some());
        let (name, _) = result.unwrap();
        assert_eq!(name, "my-svc");
    }

    #[test]
    fn selector_is_subset_of_pod_labels() {
        // Pod has extra labels beyond the selector — should still match.
        // Kubernetes semantics: selector is a subset check, not an equality check.
        let registry = ServiceRegistry::new();
        let svc = make_service("api-svc", "prod", &[("app", "api")]);
        registry.register(&svc).unwrap();

        // Pod has "app=api" plus extra labels
        let pod = make_pod(&[("app", "api"), ("version", "v2"), ("tier", "backend")]);
        let result = registry.find_service_for_pod(&pod);

        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "api-svc");
    }

    #[test]
    fn no_match_when_pod_missing_selector_label() {
        // Pod is missing one key from the selector → no match
        let registry = ServiceRegistry::new();
        let svc = make_service("db-svc", "default", &[("app", "db"), ("env", "prod")]);
        registry.register(&svc).unwrap();

        // Pod only has "app=db", missing "env=prod"
        let pod = make_pod(&[("app", "db")]);
        assert!(registry.find_service_for_pod(&pod).is_none());
    }

    #[test]
    fn deregister_removes_service() {
        let registry = ServiceRegistry::new();
        let svc = make_service("temp-svc", "default", &[("app", "temp")]);
        registry.register(&svc).unwrap();

        registry.deregister("temp-svc", "default").unwrap();

        let pod = make_pod(&[("app", "temp")]);
        assert!(registry.find_service_for_pod(&pod).is_none());
    }

    #[test]
    fn selector_less_service_is_not_registered() {
        // Services without selectors (e.g. ExternalName) should be silently skipped.
        let registry = ServiceRegistry::new();
        let svc = Service {
            metadata: ObjectMeta {
                name: Some("headless".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: None, // no selector
                ..Default::default()
            }),
            ..Default::default()
        };
        registry.register(&svc).unwrap();

        // Registry should be empty — nothing was registered
        let pod = make_pod(&[("app", "anything")]);
        assert!(registry.find_service_for_pod(&pod).is_none());
    }
}
