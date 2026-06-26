mod ch_client;
mod config;
mod grpc_client;
mod tools;

use anyhow::Result;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use crate::ch_client::ChClient;
use crate::config::load_config;
use crate::grpc_client::GrpcClient;
use crate::tools::QubitMcp;

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: all logging goes to stderr.
    // stdout is reserved for MCP's JSON-RPC protocol stream.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("qubit_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Qubit MCP server");

    let config = load_config()?;

    let grpc = GrpcClient::new(&config.qubit_core.grpc_address)?;
    let ch = ChClient::new(&config.clickhouse);
    let server = QubitMcp::new(grpc, ch);

    // stdio transport — Claude Code pipes JSON-RPC through stdin/stdout
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
