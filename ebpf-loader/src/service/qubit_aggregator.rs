use std::sync::Arc;

use tonic::transport::Channel;

use crate::config::EbpfLoaderConfig;
use crate::model::EbpfNetworkEvent;
use crate::proto::qubit::event_ingestion_client::EventIngestionClient;
use crate::proto::qubit::EbpfNetworkEventRequest;

pub struct QubitAggregator {
    client: EventIngestionClient<Channel>,
}

impl QubitAggregator {
    pub fn new(config: Arc<EbpfLoaderConfig>) -> Self {
        let endpoint = format!(
            "http://{}:{}",
            config.qubit_core.host, config.qubit_core.grpc_port
        );
        let channel = Channel::from_shared(endpoint)
            .expect("invalid gRPC endpoint")
            .connect_lazy();
        Self {
            client: EventIngestionClient::new(channel),
        }
    }

    pub async fn record_ebpf_event(&self, event: EbpfNetworkEvent) -> Result<(), tonic::Status> {
        let request = EbpfNetworkEventRequest {
            timestamp_ns: event.timestamp_ns,
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port as u32,
            dst_port: event.dst_port as u32,
            method: event.method,
            path: event.path,
            host: event.host,
        };

        self.client.clone().send_ebpf_network_event(request).await?;

        Ok(())
    }
}
