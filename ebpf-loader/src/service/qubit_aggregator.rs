use std::fmt::format;
use reqwest::{Client, header};
use std::sync::Arc;
use ebpf_common::DnsQueryEvent;
use serde_json::json;


use crate::config::EbpfLoaderConfig;

pub struct QubitAggregator {
    config:             Arc<EbpfLoaderConfig>,
    qubit_core_client:  Client,
}

impl QubitAggregator {
    fn new(config: Arc<>) -> Self {
        let client = Client::new();
        Self {
            config,
            qubit_core_client: client,
        }
    }

    pub async fn record_ebpf_event(&self, dns_event: DnsQueryEvent) -> Result<(), reqwest::Error> {
        let addr = format!("{}:{}", self.config.qubit_core.host, self.config.qubit_core.port);
        let response = self.qubit_core_client
            .post(addr)
            .header(header::AUTHORIZATION, "Bearer my-secret-token")
            .header(header::CONTENT_TYPE, "application/json")
            .json(json!(dns_event))
            .send()
            .await()?;
    }
    
}
