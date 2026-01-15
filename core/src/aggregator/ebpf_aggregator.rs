use crate::config::QubitConfig;
use crate::model::EbpfNetworkEvent;
use axum::Error;
use log;
use std::sync::Arc;

pub struct EbpfAggregator {
    config: Arc<QubitConfig>,
}

impl EbpfAggregator {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        Self { config }
    }

    pub fn record_ebpf_event(&self, event: EbpfNetworkEvent) -> Result<String, Error> {
        log::info!("recorded ebpf event: {}", event);
        Ok("ok".to_string())
    }
}
