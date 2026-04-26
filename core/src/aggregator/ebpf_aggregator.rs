use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use moka::sync::Cache;

use crate::aggregator::k8s_aggregator::PodInfo;
use crate::config::QubitConfig;
use crate::dao::DAO;
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
}

impl EbpfAggregator {
    pub fn new(
        config: Arc<QubitConfig>,
        db: Arc<DAO>,
        topology: Arc<RwLock<Topology>>,
        pod_cache: Arc<Cache<String, PodInfo>>,
    ) -> Self {
        Self {
            config,
            db,
            topology,
            pod_cache,
            bulk_addition_data: Arc::new(RwLock::new(Vec::new())),
            seen_edges: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn record_ebpf_event(&self, input: EbpfNetworkEventInput) -> Result<String, Error> {
        if input.path.contains("/health") {
            return Ok("Skipping health check event!".to_string());
        }

        // Convert raw u32 IPs to strings for pod cache lookup
        let src_ip = input.src_ip_str();
        let dst_ip = input.dst_ip_str();

        // Resolve source: pod cache IP → (service, namespace)
        let pod_info = self.pod_cache.get(&src_ip);
        let src_namespace = pod_info.as_ref().map(|p| p.namespace.as_str()).unwrap_or("unknown");
        let src_service = pod_info
            .as_ref()
            .filter(|p| !p.service_name.is_empty())
            .map(|p| p.service_name.as_str())
            .unwrap_or(&src_ip);

        // Resolve destination: pod cache first, fall back to Host header parsing
        let dst_pod_info = self.pod_cache.get(&dst_ip);
        let (dst_service, dst_namespace) = match dst_pod_info {
            Some(ref info) if !info.service_name.is_empty() => {
                (info.service_name.clone(), info.namespace.clone())
            }
            _ => parse_k8s_host(&input.host),
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

        // Always update the topology (it deduplicates internally via HashMap keys)
        let source_node = NodeId {
            service_name: src_service.to_string(),
            namespace: src_namespace.to_string(),
        };

        let destination_node = NodeId {
            service_name: dst_service.to_string(),
            namespace: dst_namespace.to_string(),
        };

        let flow = Flow {
            source_node: source_node.clone(),
            destination_node: destination_node.clone(),
            path: input.path,
            method: input.method,
        };

        {
            let mut topo = self
                .topology
                .write()
                .map_err(|_| Error::EbpfEventRecordingFailed("topology lock poisoned".to_string()))?;

            topo.add_flow(flow);

            topo.add_node(source_node, NodeData {
                ip: src_ip,
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

/// Parses a Kubernetes host header into (service_name, namespace).
///
/// K8s DNS names follow the pattern: `<service>.<namespace>[.svc[.cluster.local]]`
/// If the host is an IP address (e.g. kubelet health probes), we return it as-is
/// rather than splitting octets into service/namespace fields.
///
/// Examples:
///   "service-b.default.svc.cluster.local" -> ("service-b", "default")
///   "service-b.default"                   -> ("service-b", "default")
///   "service-b"                           -> ("service-b", "unknown")
///   "10.244.0.3"                          -> ("10.244.0.3", "unknown")
fn parse_k8s_host(host: &str) -> (String, String) {
    // Strip port if present (e.g. "service-b.default.svc:80")
    let host = host.split(':').next().unwrap_or(host);

    // Don't split IP addresses — they're not k8s DNS names.
    // This catches kubelet probes, direct pod-to-pod-by-IP traffic, etc.
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return (host.to_string(), "unknown".to_string());
    }

    let parts: Vec<&str> = host.splitn(3, '.').collect();
    match parts.as_slice() {
        [svc, ns, ..] => (svc.to_string(), ns.to_string()),
        [svc] => (svc.to_string(), "unknown".to_string()),
        _ => (host.to_string(), "unknown".to_string()),
    }
}
