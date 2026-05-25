use anyhow::Result;

pub mod qubit {
    tonic::include_proto!("qubit");
}

use qubit::qubit_query_client::QubitQueryClient;
use qubit::GetTopologyRequest;

/// Thin wrapper around the tonic gRPC client for Qubit's read-path.
///
/// Uses `connect_lazy()` — the actual TCP connection is established on the
/// first RPC call, not at construction time. This lets the MCP server start
/// even if core isn't running yet.
#[derive(Clone)]
pub struct GrpcClient {
    client: QubitQueryClient<tonic::transport::Channel>,
}

impl GrpcClient {
    pub fn new(address: &str) -> Result<Self> {
        let channel = tonic::transport::Channel::from_shared(address.to_string())?
            .connect_lazy();
        Ok(Self {
            client: QubitQueryClient::new(channel),
        })
    }

    pub async fn get_topology(&self) -> Result<qubit::GetTopologyResponse> {
        let response = self
            .client
            .clone()
            .get_topology(GetTopologyRequest {})
            .await?;
        Ok(response.into_inner())
    }
}
