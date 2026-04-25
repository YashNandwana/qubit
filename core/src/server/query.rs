use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tonic::{Request, Response, Status};

use crate::topology::{NodeId, Topology};

use super::qubit::qubit_query_server::QubitQuery;
use super::qubit::{
    FlowList, GetTopologyRequest, GetTopologyResponse, TopologyEdge, TopologyNode,
};

pub struct QueryServer {
    topology: Arc<RwLock<Topology>>,
}

impl QueryServer {
    pub fn new(topology: Arc<RwLock<Topology>>) -> Self {
        Self { topology }
    }
}

/// Builds a consistent map key from a NodeId: "namespace/service_name".
/// This mirrors the Topology's HashMap<NodeId, _> but with a proto-compatible string key.
fn node_key(node_id: &NodeId) -> String {
    format!("{}/{}", node_id.namespace, node_id.service_name)
}

/// Converts a Vec<Flow> into a proto FlowList.
fn to_flow_list(flows: &[crate::topology::Flow]) -> FlowList {
    FlowList {
        flows: flows
            .iter()
            .map(|flow| TopologyEdge {
                source_service: flow.source_node.service_name.clone(),
                source_namespace: flow.source_node.namespace.clone(),
                destination_service: flow.destination_node.service_name.clone(),
                destination_namespace: flow.destination_node.namespace.clone(),
                method: flow.method.clone(),
                path: flow.path.clone(),
            })
            .collect(),
    }
}

#[tonic::async_trait]
impl QubitQuery for QueryServer {
    async fn get_topology(
        &self,
        _request: Request<GetTopologyRequest>,
    ) -> Result<Response<GetTopologyResponse>, Status> {
        let topo = self
            .topology
            .read()
            .map_err(|e| Status::internal(format!("topology lock poisoned: {}", e)))?;

        let nodes: HashMap<String, TopologyNode> = topo
            .nodes
            .iter()
            .map(|(node_id, node_data)| {
                let key = node_key(node_id);
                let node = TopologyNode {
                    service_name: node_id.service_name.clone(),
                    namespace: node_id.namespace.clone(),
                    ip: node_data.ip.clone(),
                };
                (key, node)
            })
            .collect();

        // Mirrors Topology::upstream — keyed by destination: "who calls this service?"
        let upstream: HashMap<String, FlowList> = topo
            .upstream
            .iter()
            .map(|(node_id, flows)| (node_key(node_id), to_flow_list(flows)))
            .collect();

        // Mirrors Topology::downstream — keyed by source: "what does this service call?"
        let downstream: HashMap<String, FlowList> = topo
            .downstream
            .iter()
            .map(|(node_id, flows)| (node_key(node_id), to_flow_list(flows)))
            .collect();

        Ok(Response::new(GetTopologyResponse {
            nodes,
            upstream,
            downstream,
        }))
    }
}
