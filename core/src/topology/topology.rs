use std::collections::HashMap;

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct NodeId {
    pub service_name: String,
    pub namespace:    String,
}

#[derive(Clone)]
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
    pub fn resolve_unknown_node(&mut self, ip: &str, service_name: &str, namespace: &str) {
        let stale_id = NodeId {
            service_name: ip.to_string(),
            namespace: "unknown".to_string(),
        };

        // Nothing to fix if no stale node exists for this IP
        if !self.nodes.contains_key(&stale_id) {
            return;
        }

        let resolved_id = NodeId {
            service_name: service_name.to_string(),
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
