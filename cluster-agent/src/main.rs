mod config;
mod kubernetes;
mod proto;
mod server;
mod service;

use std::sync::Arc;

use kube::Client;

use crate::config::init_config;
use crate::service::ClusterAggregator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let config = init_config();
    log::info!("Config loaded: {:?}", config);

    let aggregator = Arc::new(ClusterAggregator::new(config.clone()));

    let kube_cfg = match kube::Config::incluster() {
        Ok(cfg) => cfg,
        Err(_) => kube::Config::infer()
            .await
            .map_err(|e| anyhow::anyhow!("failed to infer kube config: {}", e))?,
    };
    let client = Client::try_from(kube_cfg)
        .map_err(|e| anyhow::anyhow!("failed to create kube client: {}", e))?;

    server::run(config, client, aggregator).await
}
