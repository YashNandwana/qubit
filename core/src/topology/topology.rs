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

pub struct NodeData {
    pub ip:         String,
    pub ports:      Vec<u16>,
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
}
