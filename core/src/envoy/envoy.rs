use moka::sync::Cache;

/// Lookup table from virtual-host domain to the K8s service that owns it.
/// Populated exclusively by the cluster-agent via the `SendEnvoyRoutes` gRPC RPC
/// when it detects an `envoy.yaml` key in a ConfigMap.
pub struct EnvoyDomainCache {
    /// Value is `(service_name, namespace)`.
    inner: Cache<String, (String, String)>,
}

impl EnvoyDomainCache {
    pub fn new() -> Self {
        Self {
            inner: Cache::builder().build(),
        }
    }

    pub fn get(&self, domain: &str) -> Option<(String, String)> {
        self.inner.get(domain)
    }

    pub fn insert(&self, domain: String, service_name: String, namespace: String) {
        self.inner.insert(domain, (service_name, namespace));
    }
}
