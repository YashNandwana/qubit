use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::model::EbpfNetworkEvent;
use crate::model::Error;
use log;
use std::sync::Arc;

pub struct EbpfAggregator {
    config: Arc<QubitConfig>,
    db: Arc<DAO>,
}

impl EbpfAggregator {
    pub fn new(config: Arc<QubitConfig>, db: Arc<DAO>) -> Self {
        Self { config, db }
    }

    pub async fn record_ebpf_event(
        &self,
        event: EbpfNetworkEvent) -> Result<String, Error> {
        log::info!("recorded ebpf event: {}", event);
        self.db.add_event(event)
            .await
            .map_err(|e| Error::EbpfEventRecordingFailed(e.to_string()))?;
        Ok("saved event!".to_string())
    }
}
