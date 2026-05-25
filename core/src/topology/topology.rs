use std::collections::HashMap;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct NodeId {
    pub application_name: String,
    pub namespace:        String,
}

#[derive(Clone, Debug)]
pub struct Flow {
    pub source_node:      NodeId,
    pub destination_node: NodeId,
    pub path:             String,
    pub method:           String,
}

pub struct Topology {
    pub nodes:      HashMap<NodeId, NodeData>,
    /// Indexed by destination — "who calls this service?"
    pub upstream:   HashMap<NodeId, Vec<Flow>>,
    /// Indexed by source — "what does this service call?"
    pub downstream: HashMap<NodeId, Vec<Flow>>,
}

#[derive(Default)]
pub struct NodeData {
    pub ip:         String,
    pub k8s_events: Vec<K8sEvent>,
}

pub struct K8sEvent {
    pub timestamp:     u64,
    pub resource_name: String,
    pub resource_type: String,
    pub event_type:    String,
    pub event_data:    String,
}

impl Topology {
    pub fn new() -> Self {
        Self {
            nodes:      HashMap::new(),
            upstream:   HashMap::new(),
            downstream: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: NodeId, node_data: NodeData) {
        if self.nodes.contains_key(&node) {
            return;
        }
        self.nodes.insert(node, node_data);
    }

    pub fn add_flow(&mut self, flow: Flow) {
        self.upstream.entry(flow.destination_node.clone()).or_default().push(flow.clone());
        self.downstream.entry(flow.source_node.clone()).or_default().push(flow);
    }

    pub fn add_k8s_event(&mut self, node: NodeId, event: K8sEvent) {
        if let Some(node_data) = self.nodes.get_mut(&node) {
            node_data.k8s_events.push(event);
        }
    }

    pub fn get_topology(&self) -> &Topology {
        return &self;
    }

    /// When a pod-cache mapping arrives (ip → service/namespace), fix any
    /// stale topology node that was created with the raw IP before the
    /// mapping existed. Rewrites the node key and every flow reference.
    pub fn resolve_unknown_node(&mut self, ip: &str, application_name: &str, namespace: &str) {
        let stale_id = NodeId {
            application_name: ip.to_string(),
            namespace: "unknown".to_string(),
        };

        // Nothing to fix if no stale node exists for this IP
        if !self.nodes.contains_key(&stale_id) {
            return;
        }

        let resolved_id = NodeId {
            application_name: application_name.to_string(),
            namespace: namespace.to_string(),
        };

        // 1. Move node data from stale key → resolved key.
        //    `remove` returns Option<V> — we take ownership of the value
        //    and insert it under the new key. `or_insert` avoids overwriting
        //    if a correct node already exists from later events.
        if let Some(node_data) = self.nodes.remove(&stale_id) {
            self.nodes.entry(resolved_id.clone()).or_insert(node_data);
        }

        // 2. Re-key upstream map (keyed by destination).
        //    If the stale node was a destination, move its entry.
        if let Some(flows) = self.upstream.remove(&stale_id) {
            self.upstream.entry(resolved_id.clone()).or_default().extend(flows);
        }

        // 3. Re-key downstream map (keyed by source).
        if let Some(flows) = self.downstream.remove(&stale_id) {
            self.downstream.entry(resolved_id.clone()).or_default().extend(flows);
        }

        // 4. Update flow references everywhere — the stale NodeId might appear
        //    as source or destination inside flows belonging to OTHER nodes.
        for flows in self.upstream.values_mut() {
            for flow in flows.iter_mut() {
                if flow.source_node == stale_id {
                    flow.source_node = resolved_id.clone();
                }
                if flow.destination_node == stale_id {
                    flow.destination_node = resolved_id.clone();
                }
            }
        }
        for flows in self.downstream.values_mut() {
            for flow in flows.iter_mut() {
                if flow.source_node == stale_id {
                    flow.source_node = resolved_id.clone();
                }
                if flow.destination_node == stale_id {
                    flow.destination_node = resolved_id.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(svc: &str, ns: &str) -> NodeId {
        NodeId { application_name: svc.to_string(), namespace: ns.to_string() }
    }

    fn flow(src_svc: &str, src_ns: &str, dst_svc: &str, dst_ns: &str) -> Flow {
        Flow {
            source_node: node(src_svc, src_ns),
            destination_node: node(dst_svc, dst_ns),
            path: "/".to_string(),
            method: "GET".to_string(),
        }
    }

    #[test]
    fn add_node_is_idempotent() {
        // Adding the same NodeId twice must not overwrite — topology.add_node
        // is a no-op when the key already exists (early return on contains_key).
        let mut topo = Topology::new();
        let id = node("svc-a", "default");

        topo.add_node(id.clone(), NodeData { ip: "10.0.0.1".to_string(), ..Default::default() });
        topo.add_node(id.clone(), NodeData { ip: "10.0.0.2".to_string(), ..Default::default() }); // should be ignored

        assert_eq!(topo.nodes.len(), 1);
        assert_eq!(topo.nodes[&id].ip, "10.0.0.1"); // first value preserved
    }

    #[test]
    fn add_flow_populates_both_maps() {
        // A flow from A→B must appear in:
        //   downstream[A] (what A calls)
        //   upstream[B]   (who calls B)
        let mut topo = Topology::new();
        let a = node("svc-a", "default");
        let b = node("svc-b", "default");

        topo.add_flow(flow("svc-a", "default", "svc-b", "default"));

        assert_eq!(topo.downstream[&a].len(), 1);
        assert_eq!(topo.upstream[&b].len(), 1);
        assert_eq!(topo.downstream[&a][0].destination_node, b);
        assert_eq!(topo.upstream[&b][0].source_node, a);
    }

    #[test]
    fn resolve_noop_when_no_stale_node() {
        // If no IP-based stale node exists, resolve_unknown_node is a no-op.
        let mut topo = Topology::new();
        topo.add_node(node("svc-a", "default"), NodeData::default());

        topo.resolve_unknown_node("10.0.0.99", "svc-b", "default");

        // Only the original node should exist
        assert_eq!(topo.nodes.len(), 1);
        assert!(topo.nodes.contains_key(&node("svc-a", "default")));
    }

    #[test]
    fn resolve_rewires_flows_in_other_nodes() {
        // Scenario: svc-a → 10.0.0.2 (stale) flow exists under svc-a's downstream.
        // When 10.0.0.2 is resolved to svc-b/default, the flow reference inside
        // svc-a's downstream list must be updated too.
        let mut topo = Topology::new();

        // svc-a calls an IP we haven't resolved yet
        topo.add_node(node("svc-a", "default"), NodeData { ip: "10.0.0.1".to_string(), ..Default::default() });
        topo.add_node(node("10.0.0.2", "unknown"), NodeData { ip: "10.0.0.2".to_string(), ..Default::default() });
        topo.add_flow(flow("svc-a", "default", "10.0.0.2", "unknown"));

        // Pod cache mapping arrives: 10.0.0.2 = svc-b in default
        topo.resolve_unknown_node("10.0.0.2", "svc-b", "default");

        let a = node("svc-a", "default");
        let b = node("svc-b", "default");

        // Stale node gone, resolved node present
        assert!(!topo.nodes.contains_key(&node("10.0.0.2", "unknown")));
        assert!(topo.nodes.contains_key(&b));

        // The flow in svc-a's downstream now points at the resolved destination
        assert_eq!(topo.downstream[&a][0].destination_node, b);
        // And upstream[svc-b] exists
        assert_eq!(topo.upstream[&b][0].source_node, a);
    }

    #[test]
    fn resolve_merges_when_resolved_node_already_exists() {
        // If the correct node (svc-b/default) was already created by a later
        // event that arrived before the stale one was resolved, we must not
        // overwrite it — `or_insert` in resolve_unknown_node handles this.
        let mut topo = Topology::new();

        // Correct node already exists with real IP
        topo.add_node(node("svc-b", "default"), NodeData { ip: "10.0.0.2".to_string(), ..Default::default() });
        // Stale node also exists (from an earlier unresolved event)
        topo.add_node(node("10.0.0.2", "unknown"), NodeData { ip: "10.0.0.2".to_string(), ..Default::default() });
        topo.add_flow(flow("svc-a", "default", "10.0.0.2", "unknown"));

        topo.resolve_unknown_node("10.0.0.2", "svc-b", "default");

        // Still exactly two nodes: svc-a and svc-b (stale removed)
        assert!(!topo.nodes.contains_key(&node("10.0.0.2", "unknown")));
        assert!(topo.nodes.contains_key(&node("svc-b", "default")));
        // The resolved node kept its original IP (or_insert did not overwrite)
        assert_eq!(topo.nodes[&node("svc-b", "default")].ip, "10.0.0.2");
    }
}
