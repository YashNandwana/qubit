use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::dao::DAO;
use crate::model::{EbpfNetworkEvent, K8sResourceEvent};
use crate::topology::Topology;

// ── /ping ─────────────────────────────────────────────────────────────────────

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "pong" }))
}

// ── /api/topology ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TopologyResponse {
    pub nodes: HashMap<String, NodeDto>,
    pub upstream: HashMap<String, FlowList>,
    pub downstream: HashMap<String, FlowList>,
}

#[derive(Serialize)]
pub struct NodeDto {
    #[serde(rename = "applicationName")]
    pub application_name: String,
    pub namespace: String,
    pub ip: String,
}

#[derive(Serialize)]
pub struct FlowList {
    pub flows: Vec<FlowDto>,
}

#[derive(Serialize)]
pub struct FlowDto {
    #[serde(rename = "sourceApplication")]
    pub source_application: String,
    #[serde(rename = "destinationApplication")]
    pub destination_application: String,
    pub method: String,
    pub path: String,
}

pub async fn topology(State(topology): State<Arc<RwLock<Topology>>>) -> Json<TopologyResponse> {
    let topo = topology.read().unwrap();

    let nodes = topo
        .nodes
        .iter()
        .map(|(id, data)| {
            let key = format!("{}/{}", id.namespace, id.application_name);
            let dto = NodeDto {
                application_name: id.application_name.clone(),
                namespace: id.namespace.clone(),
                ip: data.ip.clone(),
            };
            (key, dto)
        })
        .collect();

    let upstream = topo
        .upstream
        .iter()
        .map(|(id, flows)| {
            let key = format!("{}/{}", id.namespace, id.application_name);
            let list = FlowList {
                flows: flows
                    .iter()
                    .map(|f| FlowDto {
                        source_application: f.source_node.application_name.clone(),
                        destination_application: f.destination_node.application_name.clone(),
                        method: f.method.clone(),
                        path: f.path.clone(),
                    })
                    .collect(),
            };
            (key, list)
        })
        .collect();

    let downstream = topo
        .downstream
        .iter()
        .map(|(id, flows)| {
            let key = format!("{}/{}", id.namespace, id.application_name);
            let list = FlowList {
                flows: flows
                    .iter()
                    .map(|f| FlowDto {
                        source_application: f.source_node.application_name.clone(),
                        destination_application: f.destination_node.application_name.clone(),
                        method: f.method.clone(),
                        path: f.path.clone(),
                    })
                    .collect(),
            };
            (key, list)
        })
        .collect();

    Json(TopologyResponse {
        nodes,
        upstream,
        downstream,
    })
}

// ── /api/topology/subgraph ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SubgraphParams {
    pub service: String,
    pub namespace: String,
    /// BFS depth from root; defaults to 1. Send 999 from the UI for "∞".
    pub depth: Option<u32>,
}

pub async fn topology_subgraph(
    State(topology): State<Arc<RwLock<Topology>>>,
    Query(params): Query<SubgraphParams>,
) -> Json<TopologyResponse> {
    let depth = params.depth.unwrap_or(1);
    let topo = topology.read().unwrap();
    let sg = topo.get_subgraph(&params.service, &params.namespace, depth);

    let nodes: HashMap<String, NodeDto> = sg
        .nodes
        .iter()
        .filter_map(|id| {
            topo.nodes.get(id).map(|data| {
                let key = format!("{}/{}", id.namespace, id.application_name);
                let dto = NodeDto {
                    application_name: id.application_name.clone(),
                    namespace: id.namespace.clone(),
                    ip: data.ip.clone(),
                };
                (key, dto)
            })
        })
        .collect();

    let upstream: HashMap<String, FlowList> = sg
        .upstream
        .iter()
        .map(|(id, flows)| {
            let key = format!("{}/{}", id.namespace, id.application_name);
            let list = FlowList {
                flows: flows
                    .iter()
                    .map(|f| FlowDto {
                        source_application: f.source_node.application_name.clone(),
                        destination_application: f.destination_node.application_name.clone(),
                        method: f.method.clone(),
                        path: f.path.clone(),
                    })
                    .collect(),
            };
            (key, list)
        })
        .collect();

    let downstream: HashMap<String, FlowList> = sg
        .downstream
        .iter()
        .map(|(id, flows)| {
            let key = format!("{}/{}", id.namespace, id.application_name);
            let list = FlowList {
                flows: flows
                    .iter()
                    .map(|f| FlowDto {
                        source_application: f.source_node.application_name.clone(),
                        destination_application: f.destination_node.application_name.clone(),
                        method: f.method.clone(),
                        path: f.path.clone(),
                    })
                    .collect(),
            };
            (key, list)
        })
        .collect();

    Json(TopologyResponse {
        nodes,
        upstream,
        downstream,
    })
}

// ── Pagination helpers ────────────────────────────────────────────────────────

/// Query params shared by both paginated endpoints: `?page=0&page_size=50`
/// Both fields are optional — defaults are applied in each handler.
#[derive(Deserialize)]
pub struct PaginationParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

/// Generic envelope returned by both paginated endpoints.
/// The UI uses `total` and `page_size` to compute `Math.ceil(total / page_size)`
/// for the page count, without the server having to know about that presentation
/// detail.
#[derive(Serialize)]
pub struct PagedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

// ── /api/k8s-events ───────────────────────────────────────────────────────────

pub async fn k8s_events(
    State(db): State<Arc<DAO>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    // Cap at 200 so a single request can't exhaust memory.
    let page_size = params.page_size.unwrap_or(50).min(200);

    match db.get_k8s_events_paginated(page, page_size).await {
        Ok((items, total)) => {
            let body = PagedResponse::<K8sResourceEvent> {
                items,
                total,
                page,
                page_size,
            };
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            log::error!("k8s_events query failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// ── /api/network-events ───────────────────────────────────────────────────────

pub async fn network_events(
    State(db): State<Arc<DAO>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(100).min(500);

    match db.get_network_events_paginated(page, page_size).await {
        Ok((items, total)) => {
            let body = PagedResponse::<EbpfNetworkEvent> {
                items,
                total,
                page,
                page_size,
            };
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            log::error!("network_events query failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
