use crate::aggregator::EbpfAggregator;
use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::model::EbpfNetworkEvent;
use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<QubitConfig>,
    pub aggregator: Arc<EbpfAggregator>,
}

impl AppState {
    pub fn new(config: Arc<QubitConfig>, db: Arc<DAO>) -> Self {
        let aggregator = Arc::new(EbpfAggregator::new(config.clone(), db.clone()));
        Self { config, aggregator }
    }
}

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "pong" }))
}

pub async fn aggregate_ebpf_network(
    State(state): State<AppState>,
    payload: Json<EbpfNetworkEvent>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let data: EbpfNetworkEvent = payload.0;

    let aggregator = state.aggregator.clone();
    aggregator.record_ebpf_event(data)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() }))
        ))?;

    Ok(Json(json!({ "status": "ok" })))
}
