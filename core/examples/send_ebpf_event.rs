/// Simulates what ebpf-loader does: sends a gRPC event to core.
/// Usage: cargo run --example send_ebpf_event
use std::net::Ipv4Addr;

pub mod qubit {
    tonic::include_proto!("qubit");
}

use qubit::EbpfNetworkEventRequest;
use qubit::event_ingestion_client::EventIngestionClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = EventIngestionClient::connect("http://localhost:50051").await?;

    let request = EbpfNetworkEventRequest {
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos() as u64,
        src_ip: u32::from(Ipv4Addr::new(10, 0, 0, 1)).to_be(),
        dst_ip: u32::from(Ipv4Addr::new(10, 0, 0, 2)).to_be(),
        src_port: 54321,
        dst_port: 8080,
        method: "GET".to_string(),
        path: "/api/v1/users".to_string(),
        host: "service-b.default.svc.cluster.local".to_string(),
    };

    let response = client.send_ebpf_network_event(request).await?;
    println!("Response: {:?}", response.into_inner());

    Ok(())
}
