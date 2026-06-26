use std::collections::{HashMap, HashSet};
use std::fmt;

/// Uniquely identifies a service node. `application_name` temporarily holds a
/// raw IP when the pod cache hasn't resolved it to a service name yet.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct NodeId {
    pub application_name: String,
    pub namespace: String,
}

/// One observed HTTP call. Deduplicated by (src, dst, path, method) before
/// being added to the graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    pub source_node: NodeId,
    pub destination_node: NodeId,
    pub path: String,
    pub method: String,
}

pub struct Topology {
    pub nodes: HashMap<NodeId, NodeData>,
    /// Indexed by destination — "who calls this service?"
    pub upstream: HashMap<NodeId, Vec<Flow>>,
    /// Indexed by source — "what does this service call?"
    pub downstream: HashMap<NodeId, Vec<Flow>>,
}

#[derive(Default)]
pub struct NodeData {
    pub ip: String,
    /// K8s resource events (Deployment, Ingress, HPA, etc.) attached to this node.
    pub k8s_events: Vec<K8sEvent>,
}

pub struct K8sEvent {
    pub timestamp: u64,
    pub resource_name: String,
    pub resource_type: String,
    pub event_type: String,
    pub event_data: String,
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.application_name)
    }
}

impl fmt::Display for Flow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} → {}  {} {}",
            self.source_node, self.destination_node, self.method, self.path
        )
    }
}

impl fmt::Display for Topology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let edge_count: usize = self.downstream.values().map(|v| v.len()).sum();
        writeln!(
            f,
            "Topology: {} nodes, {} edges",
            self.nodes.len(),
            edge_count
        )?;
        // Sort by source node key for deterministic output.
        let mut sources: Vec<&NodeId> = self.downstream.keys().collect();
        sources.sort_by_key(|n| format!("{}", n));
        for src in sources {
            for flow in &self.downstream[src] {
                writeln!(f, "  {}", flow)?;
            }
        }
        Ok(())
    }
}

impl Topology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            upstream: HashMap::new(),
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
        let upstream_flows = self
            .upstream
            .entry(flow.destination_node.clone())
            .or_default();
        if !upstream_flows.contains(&flow) {
            upstream_flows.push(flow.clone());
        }

        let downstream_flows = self.downstream.entry(flow.source_node.clone()).or_default();
        if !downstream_flows.contains(&flow) {
            downstream_flows.push(flow);
        }
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
            self.upstream
                .entry(resolved_id.clone())
                .or_default()
                .extend(flows);
        }

        // 3. Re-key downstream map (keyed by source).
        if let Some(flows) = self.downstream.remove(&stale_id) {
            self.downstream
                .entry(resolved_id.clone())
                .or_default()
                .extend(flows);
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

/// Filtered view of the topology rooted at a single service.
pub struct SubgraphResult {
    pub nodes: HashSet<NodeId>,
    /// Keyed by destination node — "who calls this service?" (BFS-tree edges only for non-root).
    pub upstream: HashMap<NodeId, Vec<Flow>>,
    /// Keyed by source node — "what does this service call?" (BFS-tree edges only for non-root).
    pub downstream: HashMap<NodeId, Vec<Flow>>,
}

impl Topology {
    /// BFS from `root` following downstream edges only up to `depth` hops.
    ///
    /// Root node: all incoming + all outgoing edges included.
    /// All other visible nodes: only the single BFS-tree edge (parent → child) — cross-edges
    /// between siblings are excluded so the graph stays unambiguous.
    /// Callers of root are added as nodes but not expanded further.
    pub fn get_subgraph(&self, root_app: &str, root_ns: &str, depth: u32) -> SubgraphResult {
        let root = NodeId {
            application_name: root_app.to_string(),
            namespace: root_ns.to_string(),
        };

        if !self.nodes.contains_key(&root) {
            return SubgraphResult {
                nodes: HashSet::new(),
                upstream: HashMap::new(),
                downstream: HashMap::new(),
            };
        }

        // BFS downstream from root up to `depth` hops.
        // bfs_edges records (parent, child) pairs — the spanning tree, not cross-edges.
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut bfs_edges: HashSet<(NodeId, NodeId)> = HashSet::new();
        visited.insert(root.clone());
        let mut frontier = vec![root.clone()];

        for _ in 0..depth {
            if frontier.is_empty() {
                break;
            }
            let mut next: Vec<NodeId> = Vec::new();
            for node in &frontier {
                if let Some(flows) = self.downstream.get(node) {
                    for flow in flows {
                        let dst = flow.destination_node.clone();
                        if !visited.contains(&dst) {
                            visited.insert(dst.clone());
                            bfs_edges.insert((node.clone(), dst.clone()));
                            next.push(dst);
                        }
                    }
                }
            }
            frontier = next;
        }

        // Root callers: included as nodes but not expanded.
        let mut all_nodes = visited.clone();
        if let Some(inflows) = self.upstream.get(&root) {
            for flow in inflows {
                all_nodes.insert(flow.source_node.clone());
            }
        }

        // Build visible edge sets.
        let mut vis_upstream: HashMap<NodeId, Vec<Flow>> = HashMap::new();
        let mut vis_downstream: HashMap<NodeId, Vec<Flow>> = HashMap::new();

        // Root gets everything.
        if let Some(inflows) = self.upstream.get(&root) {
            vis_upstream.insert(root.clone(), inflows.clone());
        }
        if let Some(outflows) = self.downstream.get(&root) {
            vis_downstream.insert(root.clone(), outflows.clone());
        }

        // Non-root BFS nodes: only BFS-tree edges.
        for node in &visited {
            if node == &root {
                continue;
            }

            let in_flows: Vec<Flow> = self
                .upstream
                .get(node)
                .map(|flows| {
                    flows
                        .iter()
                        .filter(|f| bfs_edges.contains(&(f.source_node.clone(), node.clone())))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            if !in_flows.is_empty() {
                vis_upstream.insert(node.clone(), in_flows);
            }

            let out_flows: Vec<Flow> = self
                .downstream
                .get(node)
                .map(|flows| {
                    flows
                        .iter()
                        .filter(|f| bfs_edges.contains(&(node.clone(), f.destination_node.clone())))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            if !out_flows.is_empty() {
                vis_downstream.insert(node.clone(), out_flows);
            }
        }

        SubgraphResult {
            nodes: all_nodes,
            upstream: vis_upstream,
            downstream: vis_downstream,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(svc: &str, ns: &str) -> NodeId {
        NodeId {
            application_name: svc.to_string(),
            namespace: ns.to_string(),
        }
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

        topo.add_node(
            id.clone(),
            NodeData {
                ip: "10.0.0.1".to_string(),
                ..Default::default()
            },
        );
        topo.add_node(
            id.clone(),
            NodeData {
                ip: "10.0.0.2".to_string(),
                ..Default::default()
            },
        ); // should be ignored

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
        topo.add_node(
            node("svc-a", "default"),
            NodeData {
                ip: "10.0.0.1".to_string(),
                ..Default::default()
            },
        );
        topo.add_node(
            node("10.0.0.2", "unknown"),
            NodeData {
                ip: "10.0.0.2".to_string(),
                ..Default::default()
            },
        );
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
        topo.add_node(
            node("svc-b", "default"),
            NodeData {
                ip: "10.0.0.2".to_string(),
                ..Default::default()
            },
        );
        // Stale node also exists (from an earlier unresolved event)
        topo.add_node(
            node("10.0.0.2", "unknown"),
            NodeData {
                ip: "10.0.0.2".to_string(),
                ..Default::default()
            },
        );
        topo.add_flow(flow("svc-a", "default", "10.0.0.2", "unknown"));

        topo.resolve_unknown_node("10.0.0.2", "svc-b", "default");

        // Still exactly two nodes: svc-a and svc-b (stale removed)
        assert!(!topo.nodes.contains_key(&node("10.0.0.2", "unknown")));
        assert!(topo.nodes.contains_key(&node("svc-b", "default")));
        // The resolved node kept its original IP (or_insert did not overwrite)
        assert_eq!(topo.nodes[&node("svc-b", "default")].ip, "10.0.0.2");
    }

    // ── get_subgraph tests ────────────────────────────────────────────────────

    fn make_graph() -> Topology {
        // Graph: caller → A → B → C, A → D
        //                      └──── E  (B also calls E)
        // caller calls A (so caller is a root-caller node)
        let mut topo = Topology::new();
        for svc in &["caller", "svc-a", "svc-b", "svc-c", "svc-d", "svc-e"] {
            topo.add_node(node(svc, "ns"), NodeData::default());
        }
        topo.add_flow(flow("caller", "ns", "svc-a", "ns"));
        topo.add_flow(flow("svc-a", "ns", "svc-b", "ns"));
        topo.add_flow(flow("svc-a", "ns", "svc-d", "ns"));
        topo.add_flow(flow("svc-b", "ns", "svc-c", "ns"));
        topo.add_flow(flow("svc-b", "ns", "svc-e", "ns"));
        topo
    }

    #[test]
    fn subgraph_depth_1_includes_direct_callees_and_root_caller() {
        let topo = make_graph();
        let sg = topo.get_subgraph("svc-a", "ns", 1);

        // Root (svc-a) + its direct callees (svc-b, svc-d) + its caller (caller)
        assert!(sg.nodes.contains(&node("svc-a", "ns")));
        assert!(sg.nodes.contains(&node("svc-b", "ns")));
        assert!(sg.nodes.contains(&node("svc-d", "ns")));
        assert!(sg.nodes.contains(&node("caller", "ns")));
        // Level-2 nodes not reached yet
        assert!(!sg.nodes.contains(&node("svc-c", "ns")));
        assert!(!sg.nodes.contains(&node("svc-e", "ns")));
    }

    #[test]
    fn subgraph_depth_2_reaches_level_2_nodes() {
        let topo = make_graph();
        let sg = topo.get_subgraph("svc-a", "ns", 2);

        for svc in &["svc-a", "svc-b", "svc-c", "svc-d", "svc-e", "caller"] {
            assert!(
                sg.nodes.contains(&node(svc, "ns")),
                "{} not in subgraph",
                svc
            );
        }
    }

    #[test]
    fn subgraph_cross_edges_excluded_for_non_root() {
        // Add a cross-edge: svc-d → svc-b (sibling of svc-b, both children of svc-a)
        let mut topo = make_graph();
        topo.add_flow(flow("svc-d", "ns", "svc-b", "ns"));

        let sg = topo.get_subgraph("svc-a", "ns", 1);

        // svc-d is in the subgraph (direct callee of root)
        assert!(sg.nodes.contains(&node("svc-d", "ns")));
        // But the cross-edge svc-d → svc-b must NOT appear in svc-d's downstream
        // (it's not a BFS tree edge — svc-b was discovered from svc-a, not svc-d)
        let svc_d_out = sg.downstream.get(&node("svc-d", "ns"));
        assert!(
            svc_d_out.is_none()
                || svc_d_out
                    .unwrap()
                    .iter()
                    .all(|f| f.destination_node != node("svc-b", "ns")),
            "cross-edge svc-d→svc-b should be filtered"
        );
    }

    #[test]
    fn subgraph_unknown_root_returns_empty() {
        let topo = make_graph();
        let sg = topo.get_subgraph("does-not-exist", "ns", 2);
        assert!(sg.nodes.is_empty());
    }
}
