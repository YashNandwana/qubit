use std::sync::{Arc, RwLock};
use tokio::signal;
use tracing::info;

mod aggregator;
mod config;
mod dao;
mod model;
mod server;
mod topology;

use crate::config::init_config;
use crate::dao::DAO;
use crate::server::ServerFactory;
use crate::topology::Topology;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let config: Arc<config::QubitConfig> = init_config();
    info!("Config loaded: {:?}", config);

    let db = Arc::new(DAO::new(config.clone()).map_err(|e| anyhow::anyhow!(e))?);
    db.initialize_schema().await.map_err(|e| anyhow::anyhow!(e))?;
    info!("DB initialized");

    let topology = Arc::new(RwLock::new(Topology::new()));

    let factory = ServerFactory::new(config.clone(), db.clone(), topology.clone());
    let http = factory.http();
    let grpc = factory.grpc();

    let mut http_handle = tokio::spawn(async move {
        http.do_serve().await.map_err(|e| e.to_string())
    });

    let mut grpc_handle = tokio::spawn(async move {
        grpc.do_serve().await
    });

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("shutdown signal received");
        }
        res = &mut http_handle => {
            info!("http server task finished: {:?}", res);
        }
        res = &mut grpc_handle => {
            info!("grpc server task finished: {:?}", res);
        }
    }

    let _ = http_handle.abort();

    Ok(())
}
