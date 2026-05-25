use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use moka::sync::Cache;

use crate::aggregator::k8s_aggregator::PodInfo;
use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::envoy::EnvoyDomainCache;
use crate::model::{EbpfNetworkEvent, EbpfNetworkEventInput, Error};
use crate::topology::{Flow, NodeData, NodeId, Topology};

pub struct EbpfAggregator {
    config: Arc<QubitConfig>,
    db: Arc<DAO>,
    topology: Arc<RwLock<Topology>>,
    pod_cache: Arc<Cache<String, PodInfo>>,
    bulk_addition_data: Arc<RwLock<Vec<EbpfNetworkEvent>>>,
    /// Dedup cache: tracks which service→service pairs we've already stored.
    /// Key: "src_ns/src_svc", Value: set of "dst_ns/dst_svc".
    /// Prevents duplicate DB rows for the same dependency edge.
    seen_edges: Arc<RwLock<HashSet<String>>>,
    envoy_cache: Arc<EnvoyDomainCache>,
}

impl EbpfAggregator {
    pub fn new(
        config: Arc<QubitConfig>,
        db: Arc<DAO>,
        topology: Arc<RwLock<Topology>>,
        pod_cache: Arc<Cache<String, PodInfo>>,
        envoy_cache: Arc<EnvoyDomainCache>
    ) -> Self {
        Self {
            config,
            db,
            topology,
            pod_cache,
            bulk_addition_data: Arc::new(RwLock::new(Vec::new())),
            seen_edges: Arc::new(RwLock::new(HashSet::new())),
            envoy_cache,
        }
    }

    pub async fn record_ebpf_event(&self, input: EbpfNetworkEventInput) -> Result<String, Error> {
        if input.path.contains("/health") {
            return Ok("Skipping health check event!".to_string());
        }

        // Convert raw u32 IPs to strings for pod cache lookup
        let src_ip = input.src_ip_str();
        let dst_ip = input.dst_ip_str();

        // Resolve source: pod cache IP → (service, namespace).
        // If the pod cache hasn't seen this IP yet (cluster-agent event hasn't arrived),
        // drop the event. eBPF traffic is continuous — the next packet from this pod will
        // resolve correctly once the cache is populated. This avoids raw IPs leaking into
        // the topology during the brief startup race window.
        let pod_info = self.pod_cache.get(&src_ip);
        let (src_namespace, src_service_owned);
        match pod_info.as_ref().filter(|p| !p.application_name.is_empty()) {
            Some(p) => {
                src_namespace = p.namespace.clone();
                src_service_owned = p.application_name.clone();
            }
            None => {
                log::debug!("dropping event: source IP {} not yet in pod cache", src_ip);
                return Ok("unresolved source — waiting for pod event".to_string());
            }
        }
        let src_service = src_service_owned.as_str();

        // Strip port from Host header before cache lookups — curl sends "host:port",
        // but cache keys are always bare hostnames.
        let host = input.host.split(':').next().unwrap_or(&input.host);

        // Resolve destination (priority order):
        // 1. Envoy cache — authoritative; maps Host header to service/namespace for
        //    any service Envoy knows about (both .svc FQDNs and custom domains like svc.meesho.int)
        // 2. Pod cache — fallback for direct pod-to-pod traffic not routed through Envoy
        // 3. parse_k8s_host — heuristic last resort for K8s DNS names we can pattern-match
        let (dst_service, dst_namespace) = if let Some((svc, ns)) = self.envoy_cache.get(host) {
            (svc, ns)
        } else {
            let dst_pod_info = self.pod_cache.get(&dst_ip);
            match dst_pod_info {
                Some(ref info) if !info.application_name.is_empty() => {
                    (info.application_name.clone(), info.namespace.clone())
                }
                _ => parse_k8s_host(host),
            }
        };

        // Dedup: only persist one row per (src_service → dst_service) pair.
        // The topology still gets the flow, but we don't flood ClickHouse
        // with thousands of identical dependency edges.
        let edge_key = format!(
            "{}/{} -> {}/{}",
            src_namespace, src_service, dst_namespace, dst_service
        );

        let is_new_edge = {
            let mut seen = self.seen_edges
                .write()
                .map_err(|_| Error::EbpfEventRecordingFailed("seen_edges lock poisoned".to_string()))?;
            seen.insert(edge_key)
        };

        if is_new_edge {
            let db_event = EbpfNetworkEvent {
                timestamp_ns: input.timestamp_ns,
                src_service: src_service.to_string(),
                src_namespace: src_namespace.to_string(),
                dst_service: dst_service.clone(),
                dst_namespace: dst_namespace.clone(),
                src_port: input.src_port,
                dst_port: input.dst_port,
                method: input.method.clone(),
                path: input.path.clone(),
                host: input.host.clone(),
            };

            log::debug!("new edge: {}", db_event);
            self.insert_bulk_events(db_event).await?;
        }

        let source_node = NodeId {
            application_name: src_service.to_string(),
            namespace: src_namespace.to_string(),
        };

        let destination_node = NodeId {
            application_name: dst_service.to_string(),
            namespace: dst_namespace.to_string(),
        };

        {
            let mut topo = self
                .topology
                .write()
                .map_err(|_| Error::EbpfEventRecordingFailed("topology lock poisoned".to_string()))?;

            // Only add the flow on new edges — topology needs one edge per service pair,
            // not one per captured packet.
            if is_new_edge {
                let flow = Flow {
                    source_node: source_node.clone(),
                    destination_node: destination_node.clone(),
                    path: input.path,
                    method: input.method,
                };
                topo.add_flow(flow);
            }

            topo.add_node(source_node, NodeData {
                ip: src_ip.clone(),
                ..Default::default()
            });

            topo.add_node(destination_node, NodeData {
                ip: dst_ip,
                ..Default::default()
            });
        }

        Ok("saved event!".to_string())
    }

    async fn insert_bulk_events(&self, event: EbpfNetworkEvent) -> Result<(), Error> {
        // Single write lock — push, check length, and conditionally drain in one acquisition.
        // Taking a second read/write lock on the same RwLock while holding one = deadlock.
        let events_to_flush = {
            let mut buffer = self.bulk_addition_data
                .write()
                .map_err(|_| Error::EbpfEventRecordingFailed("bulk data lock poisoned".to_string()))?;

            buffer.push(event);

            if buffer.len() >= self.config.app.ebpf_bulk_insertion_max_size as usize {
                Some(buffer.drain(..).collect::<Vec<_>>())
            } else {
                None
            }
        }; // write lock dropped here before the async DB call

        if let Some(events) = events_to_flush {
            self.db
                .add_events(events)
                .await
                .map_err(|e| Error::EbpfEventRecordingFailed(e.to_string()))?;
        }

        Ok(())
    }

    /// Spawns a background task that flushes the bulk buffer on a fixed interval.
    /// This ensures events reach ClickHouse even when traffic is low and the
    /// buffer never fills to `ebpf_bulk_insertion_max_size`.
    pub fn start_flush_timer(self: &Arc<Self>, interval_secs: u64) {
        let aggregator = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;

                let events_to_flush = {
                    let mut buffer = match aggregator.bulk_addition_data.write() {
                        Ok(b) => b,
                        Err(_) => {
                            log::error!("Flush timer: bulk data lock poisoned");
                            continue;
                        }
                    };
                    if buffer.is_empty() {
                        None
                    } else {
                        Some(buffer.drain(..).collect::<Vec<_>>())
                    }
                };

                if let Some(events) = events_to_flush {
                    log::info!("Flush timer: writing {} buffered events to DB", events.len());
                    if let Err(e) = aggregator.db.add_events(events).await {
                        log::error!("Flush timer: DB write failed: {}", e);
                    }
                }
            }
        });
    }
}

/// Parses a Kubernetes host header into (application_name, namespace).
///
/// Only trusts names that contain ".svc" as K8s internal — that substring is
/// present in every cross-namespace K8s DNS name and absent in public domains.
/// Single-label names (no dots) are also unambiguous K8s service names.
/// Everything else (e.g. "httpbin.org", "api.github.com") is treated as an
/// external host: returned as-is under the "external" namespace so it still
/// appears in the topology but doesn't pollute real namespace data.
///
/// Examples:
///   "service-b.default.svc.cluster.local" -> ("service-b", "default")
///   "service-b.default.svc"               -> ("service-b", "default")
///   "service-b.default.svc:80"            -> ("service-b", "default")
///   "service-b"                           -> ("service-b", "unknown")
///   "httpbin.org"                         -> ("httpbin.org", "external")
///   "api.github.com"                      -> ("api.github.com", "external")
///   "10.244.0.3"                          -> ("10.244.0.3", "unknown")
pub(crate) fn parse_k8s_host(host: &str) -> (String, String) {
    // Strip port if present (e.g. "service-b.default.svc:80")
    let host = host.split(':').next().unwrap_or(host);

    // IP addresses pass through — not a DNS name at all.
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return (host.to_string(), "unknown".to_string());
    }

    // Single label = unambiguous K8s service name (same namespace call).
    if !host.contains('.') {
        return (host.to_string(), "unknown".to_string());
    }

    // Only trust multi-label names that contain ".svc" — the reliable marker
    // of a K8s internal DNS name. "httpbin.org" doesn't contain it;
    // "service-b.default.svc.cluster.local" does.
    if host.contains(".svc") {
        let parts: Vec<&str> = host.splitn(3, '.').collect();
        if let [svc, ns, ..] = parts.as_slice() {
            return (svc.to_string(), ns.to_string());
        }
    }

    // External host — preserve the full name so it's visible in the topology.
    (host.to_string(), "external".to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_k8s_host;

    #[test]
    fn parse_fqdn() {
        let (svc, ns) = parse_k8s_host("service-b.default.svc.cluster.local");
        assert_eq!(svc, "service-b");
        assert_eq!(ns, "default");
    }

    #[test]
    fn parse_svc_short() {
        let (svc, ns) = parse_k8s_host("service-b.default.svc");
        assert_eq!(svc, "service-b");
        assert_eq!(ns, "default");
    }

    #[test]
    fn parse_service_only() {
        let (svc, ns) = parse_k8s_host("service-b");
        assert_eq!(svc, "service-b");
        assert_eq!(ns, "unknown");
    }

    #[test]
    fn parse_ip_passthrough() {
        let (svc, ns) = parse_k8s_host("10.244.0.3");
        assert_eq!(svc, "10.244.0.3");
        assert_eq!(ns, "unknown");
    }

    #[test]
    fn parse_strips_port() {
        let (svc, ns) = parse_k8s_host("service-b.default.svc:80");
        assert_eq!(svc, "service-b");
        assert_eq!(ns, "default");
    }

    #[test]
    fn parse_external_domain() {
        let (svc, ns) = parse_k8s_host("httpbin.org");
        assert_eq!(svc, "httpbin.org");
        assert_eq!(ns, "external");
    }

    #[test]
    fn parse_external_subdomain() {
        let (svc, ns) = parse_k8s_host("api.github.com");
        assert_eq!(svc, "api.github.com");
        assert_eq!(ns, "external");
    }
}
