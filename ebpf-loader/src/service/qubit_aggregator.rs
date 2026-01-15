use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

use crate::config::EbpfLoaderConfig;
use crate::model::EbpfNetworkEvent;

pub struct QubitAggregator {
    config: Arc<EbpfLoaderConfig>,
    qubit_core_client: Client,
}

impl QubitAggregator {
    pub fn new(config: Arc<EbpfLoaderConfig>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        Self {
            config,
            qubit_core_client: client,
        }
    }

    pub async fn record_ebpf_event(
        &self,
        ebpf_event: EbpfNetworkEvent,
    ) -> Result<(), reqwest::Error> {
        let addr = format!(
            "http://{}:{}/aggregate/ebpf/network",
            self.config.qubit_core.host, self.config.qubit_core.port
        );

        self.qubit_core_client
            .post(&addr)
            .json(&ebpf_event)
            .send()
            .await?;

        Ok(())
    }
}
