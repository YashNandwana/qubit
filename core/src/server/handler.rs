use crate::aggregator::EbpfAggregator;
use crate::config::QubitConfig;
use crate::model::EbpfNetworkEvent;
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<QubitConfig>,
    pub aggregator: Arc<EbpfAggregator>,
}

impl AppState {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        let aggregator = Arc::new(EbpfAggregator::new(config.clone()));
        Self { 
            config,
            aggregator
        }
    }
}

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "pong" }))
}

pub async fn aggregate_ebpf_network(
    State(state): State<AppState>,
    payload: Json<EbpfNetworkEvent>,
) -> Json<Value> {
    let data: EbpfNetworkEvent = payload.0;
    let aggregator = state.aggregator.clone();

    let result = match aggregator.record_ebpf_event(data) {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error: {}", e),
    };

    Json(json!({ "status": result }))
}
