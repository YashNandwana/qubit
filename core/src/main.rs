use std::sync::Arc;
use tokio::signal;
use tracing::info;

mod aggregator;
mod config;
mod dao;
mod kubernetes;
mod model;
mod server;
mod service;

use crate::config::init_config;
use crate::dao::DAO;
use crate::server::HttpServer;
use crate::service::{K8sService, K8sServiceImpl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let config: Arc<config::QubitConfig> = init_config();
    info!("Config loaded: {:?}", config);

    // initialize db
    let db = Arc::new(DAO::new(config.clone()).map_err(|e| anyhow::anyhow!(e))?);
    db.initialize_schema().await.map_err(|e| anyhow::anyhow!(e))?;
    info!("DB initialized");

    // spawn server as async task
    let server_cfg = config.clone();
    let server_db = db.clone();
    let mut server_handle = tokio::spawn(async move {
        let server: HttpServer = HttpServer::new(server_cfg, server_db);
        server.do_serve().await.map_err(|e| e.to_string())
    });

    // spawn informers as async task
    let k8s_service = K8sServiceImpl::new(config.clone(), db);
    let mut k8s_handle = tokio::spawn(async move {
        if let Err(e) = k8s_service.informer_service().await {
            log::error!("K8s service failed: {}", e);
        }
    });

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("shutdown signal received");
        }
        res = &mut server_handle => {
            info!("server task finished: {:?}", res);
        }
        res = &mut k8s_handle => {
            info!("k8s service task finished: {:?}", res);
        }
    }

    // Wait for tasks to exit cleanly (or force abort)
    let _ = server_handle.abort();

    Ok(())
}
