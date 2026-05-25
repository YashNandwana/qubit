use moka::sync::Cache;

pub struct EnvoyDomainCache {
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
