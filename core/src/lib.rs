// lib.rs — makes core a library crate in addition to a binary.
//
// Rust allows a package to have both `lib.rs` (the library) and `main.rs`
// (the binary). They are separate compilation units: main.rs can import from
// the library via the crate name (`Qubit`), while load-tests and other crates
// depend on this library via `Qubit = { path = "../core" }`.

pub mod aggregator;
pub mod config;
pub mod dao;
pub mod envoy;
pub mod model;
pub mod server;
pub mod topology;

use std::sync::{Arc, RwLock};

use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::server::ServerFactory;
use crate::topology::Topology;

/// Starts the Core gRPC and HTTP servers.
///
/// Unlike `main()`, this function does NOT handle SIGINT — the caller controls
/// the lifetime. In production, `main()` wraps this in a `tokio::select!` with
/// `signal::ctrl_c()`. In load tests, the tokio task holding this future is
/// simply aborted when the test harness drops.
pub async fn run(config: Arc<QubitConfig>) -> anyhow::Result<()> {
    let db = Arc::new(DAO::new(config.clone()).map_err(|e| anyhow::anyhow!(e))?);
    db.initialize_schema()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    log::info!("DB initialized");

    let topology = Arc::new(RwLock::new(Topology::new()));
    let cache = Arc::new(envoy::EnvoyDomainCache::new());

    let factory = ServerFactory::new(config.clone(), db.clone(), topology.clone(), cache);
    let http = factory.http();
    let grpc = factory.grpc();
    let query = factory.query();

    let mut http_handle =
        tokio::spawn(async move { http.do_serve().await.map_err(|e| e.to_string()) });

    let mut grpc_handle = tokio::spawn(async move { grpc.do_serve(query).await });

    tokio::select! {
        res = &mut http_handle => {
            log::info!("http server task finished: {:?}", res);
        }
        res = &mut grpc_handle => {
            log::info!("grpc server task finished: {:?}", res);
        }
    }

    http_handle.abort();
    grpc_handle.abort();
    Ok(())
}
