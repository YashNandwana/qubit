use std::collections::HashMap;

/// Parsed route: domain → (service_name, namespace)
pub struct EnvoyRoute {
    pub domain: String,
    pub service_name: String,
    pub namespace: String,
}

/// Parses an Envoy static YAML config and returns domain→service mappings.
///
/// Two-pass strategy mirroring the admin-API parser in core:
/// Pass 1 — `static_resources.clusters[]` → cluster_name → (service_name, namespace)
///   Uses the upstream endpoint FQDN (socket_address.address) to derive the K8s service.
///   Also inserts the raw FQDN as a key so lookups by full hostname work.
/// Pass 2 — listener route config virtual hosts → non-wildcard domains
///   Resolves each virtual host's upstream cluster name through the map from Pass 1.
pub fn parse_envoy_routes(yaml_str: &str) -> Vec<EnvoyRoute> {
    let value: serde_yaml::Value = match serde_yaml::from_str(yaml_str) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse envoy.yaml: {}", e);
            return Vec::new();
        }
    };

    let mut routes = Vec::new();

    // Pass 1: build cluster_name → (service_name, namespace) from upstream FQDNs
    let mut cluster_map: HashMap<String, (String, String)> = HashMap::new();
    if let Some(clusters) = value["static_resources"]["clusters"].as_sequence() {
        for cluster in clusters {
            let name = match cluster["name"].as_str() {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            let addr = cluster["load_assignment"]["endpoints"][0]["lb_endpoints"][0]
                ["endpoint"]["address"]["socket_address"]["address"]
                .as_str()
                .unwrap_or("");

            if let Some((svc, ns)) = svc_namespace_from_fqdn(addr) {
                // Insert the FQDN itself so lookups by full hostname resolve.
                routes.push(EnvoyRoute { domain: addr.to_string(), service_name: svc.clone(), namespace: ns.clone() });
                cluster_map.insert(name.to_string(), (svc, ns));
            }
        }
    }

    // Pass 2: walk listener filter chain route configs for virtual host domains
    let listeners = match value["static_resources"]["listeners"].as_sequence() {
        Some(l) => l,
        None => return routes,
    };

    for listener in listeners {
        let filter_chains = match listener["filter_chains"].as_sequence() {
            Some(fc) => fc,
            None => continue,
        };
        for fc in filter_chains {
            let filters = match fc["filters"].as_sequence() {
                Some(f) => f,
                None => continue,
            };
            for filter in filters {
                // Route config may live directly or under typed_config
                let route_cfg = if filter["typed_config"]["route_config"].is_mapping() {
                    &filter["typed_config"]["route_config"]
                } else if filter["route_config"].is_mapping() {
                    &filter["route_config"]
                } else {
                    continue
                };

                let vhosts = match route_cfg["virtual_hosts"].as_sequence() {
                    Some(v) => v,
                    None => continue,
                };

                for vh in vhosts {
                    // Static Envoy config has exactly one upstream per virtual host,
                    // so the first route's cluster is the only relevant one.
                    let cluster_name = vh["routes"][0]["route"]["cluster"].as_str().unwrap_or("");
                    let mapping = match cluster_map.get(cluster_name) {
                        Some(m) => m.clone(),
                        None => continue,
                    };

                    let domains = match vh["domains"].as_sequence() {
                        Some(d) => d,
                        None => continue,
                    };

                    for domain in domains {
                        if let Some(d) = domain.as_str() {
                            // Wildcards can't be used as exact lookup keys.
                            if d == "*" || d.starts_with("*.") {
                                continue;
                            }
                            routes.push(EnvoyRoute {
                                domain: d.to_string(),
                                service_name: mapping.0.clone(),
                                namespace: mapping.1.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    routes
}

/// "service-b.default.svc.cluster.local" → Some(("service-b", "default"))
fn svc_namespace_from_fqdn(host: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = host.split('.').collect();
    let svc_pos = parts.iter().position(|&p| p == "svc")?;
    if svc_pos < 2 {
        return None;
    }
    Some((parts[svc_pos - 2].to_string(), parts[svc_pos - 1].to_string()))
}
