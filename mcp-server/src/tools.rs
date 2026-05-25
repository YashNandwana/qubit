use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use rmcp::model::{ServerCapabilities, ServerInfo, Implementation};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ch_client::ChClient;
use crate::grpc_client::GrpcClient;

/// MCP server that exposes Qubit's topology and event data as tools.
#[derive(Clone)]
pub struct QubitMcp {
    grpc: GrpcClient,
    ch: ChClient,
    tool_router: ToolRouter<QubitMcp>,
}

impl QubitMcp {
    pub fn new(grpc: GrpcClient, ch: ChClient) -> Self {
        Self {
            grpc,
            ch,
            tool_router: Self::tool_router(),
        }
    }
}

// ── Tool parameter types ────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetServiceDepsInput {
    /// Service name to look up
    pub service: String,
    /// Kubernetes namespace
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetK8sEventsInput {
    /// Filter by namespace (optional)
    pub namespace: Option<String>,
    /// Filter by resource type, e.g. "Deployment", "Event", "HPA" (optional)
    pub resource_type: Option<String>,
    /// Look back N minutes (default 60)
    pub last_minutes: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNetworkEventsInput {
    /// Filter by source service name (optional)
    pub src_service: Option<String>,
    /// Filter by destination service name (optional)
    pub dst_service: Option<String>,
    /// Look back N minutes (default 60)
    pub last_minutes: Option<u32>,
}

// ── Tool implementations ────────────────────────────────────────────

#[tool_router]
impl QubitMcp {
    /// Returns the full service topology — all discovered services and their
    /// HTTP dependencies as captured by the eBPF probe.
    #[tool(description = "Get the full service topology — all services and their HTTP dependencies")]
    async fn get_topology(&self) -> String {
        let topo = match self.grpc.get_topology().await {
            Ok(t) => t,
            Err(e) => return format!("Error fetching topology: {}", e),
        };

        if topo.nodes.is_empty() {
            return "No services discovered yet. The cluster may not have traffic flowing.".to_string();
        }

        let mut out = format!("Service Topology ({} services):\n", topo.nodes.len());

        let mut keys: Vec<&String> = topo.nodes.keys().collect();
        keys.sort();

        for key in keys {
            let node = &topo.nodes[key];
            out.push_str(&format!("\n  {}/{}", node.namespace, node.application_name));
            if !node.ip.is_empty() {
                out.push_str(&format!("  (ip: {})", node.ip));
            }
            out.push('\n');

            if let Some(flow_list) = topo.downstream.get(key) {
                for edge in &flow_list.flows {
                    out.push_str(&format!(
                        "    → {} {} → {}/{}\n",
                        edge.method, edge.path,
                        edge.destination_namespace, edge.destination_application
                    ));
                }
            }
        }

        out
    }

    /// Returns upstream (who calls it) and downstream (what it calls)
    /// dependencies for a specific service.
    #[tool(description = "Get upstream and downstream dependencies for a specific service")]
    async fn get_service_dependencies(
        &self,
        Parameters(input): Parameters<GetServiceDepsInput>,
    ) -> String {
        let topo = match self.grpc.get_topology().await {
            Ok(t) => t,
            Err(e) => return format!("Error fetching topology: {}", e),
        };
        let key = format!("{}/{}", input.namespace, input.service);

        if !topo.nodes.contains_key(&key) {
            return format!(
                "Service '{}' not found in topology. Known services: {}",
                key,
                topo.nodes.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        let mut out = format!("Dependencies for {}:\n", key);

        out.push_str("\n  Upstream (callers):\n");
        match topo.upstream.get(&key) {
            Some(flow_list) if !flow_list.flows.is_empty() => {
                for edge in &flow_list.flows {
                    out.push_str(&format!(
                        "    ← {}/{} ({} {})\n",
                        edge.source_namespace, edge.source_application,
                        edge.method, edge.path
                    ));
                }
            }
            _ => out.push_str("    (none)\n"),
        }

        out.push_str("\n  Downstream (dependencies):\n");
        match topo.downstream.get(&key) {
            Some(flow_list) if !flow_list.flows.is_empty() => {
                for edge in &flow_list.flows {
                    out.push_str(&format!(
                        "    → {}/{} ({} {})\n",
                        edge.destination_namespace, edge.destination_application,
                        edge.method, edge.path
                    ));
                }
            }
            _ => out.push_str("    (none)\n"),
        }

        out
    }

    /// Queries recent Kubernetes resource events from ClickHouse.
    /// Returns deployments, events, HPA changes, etc. from the last N minutes.
    #[tool(description = "Query recent Kubernetes resource events (deployments, scaling events, errors, etc.)")]
    async fn get_k8s_events(
        &self,
        Parameters(input): Parameters<GetK8sEventsInput>,
    ) -> String {
        let minutes = input.last_minutes.unwrap_or(60);
        let rows = match self
            .ch
            .get_k8s_events(
                input.namespace.as_deref(),
                input.resource_type.as_deref(),
                minutes,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => return format!("Error querying K8s events: {}", e),
        };

        if rows.is_empty() {
            return format!("No K8s events found in the last {} minutes.", minutes);
        }

        let mut out = format!("K8s Events (last {} min, {} results):\n\n", minutes, rows.len());
        for row in &rows {
            out.push_str(&format!(
                "  [{}] {} {}/{} — {}\n",
                row.event_type, row.resource_type, row.namespace, row.name, row.event_time
            ));
            if !row.resource_data.is_empty() && row.resource_data != "{}" {
                out.push_str(&format!("        data: {}\n", row.resource_data));
            }
        }

        out
    }

    /// Queries eBPF-captured HTTP traffic between services from ClickHouse.
    #[tool(description = "Query eBPF-captured HTTP traffic between services")]
    async fn get_network_events(
        &self,
        Parameters(input): Parameters<GetNetworkEventsInput>,
    ) -> String {
        let minutes = input.last_minutes.unwrap_or(60);
        let rows = match self
            .ch
            .get_network_events(
                input.src_service.as_deref(),
                input.dst_service.as_deref(),
                minutes,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => return format!("Error querying network events: {}", e),
        };

        if rows.is_empty() {
            return format!("No network events found in the last {} minutes.", minutes);
        }

        let mut out = format!(
            "Network Events (last {} min, {} results):\n\n",
            minutes,
            rows.len()
        );
        for row in &rows {
            out.push_str(&format!(
                "  {}/{}:{} → {}/{}:{} | {} {} | host={}\n",
                row.src_namespace, row.src_service, row.src_port,
                row.dst_namespace, row.dst_service, row.dst_port,
                row.method, row.path, row.host
            ));
        }

        out
    }
}

// ── ServerHandler — tells MCP clients who we are ────────────────────
//
// `#[tool_handler]` auto-generates `call_tool` and `list_tools` methods
// by delegating to `self.tool_router`. We only need to override `get_info`
// to provide the server name, instructions, and declare that we support tools.

#[tool_handler]
impl ServerHandler for QubitMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Qubit — eBPF-powered service dependency mapper for Kubernetes. \
                 Use get_topology to see all services and their HTTP dependencies. \
                 Use get_service_dependencies to drill into a specific service. \
                 Use get_k8s_events to see recent Kubernetes resource events. \
                 Use get_network_events to see raw eBPF-captured HTTP traffic."
                    .to_string(),
            ),
            capabilities: ServerCapabilities {
                tools: Some(Default::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "qubit".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }
}
