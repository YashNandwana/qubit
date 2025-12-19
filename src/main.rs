use std::ptr::null;
use std::sync::Arc;
use axum::serve;
use serde::de::Unexpected::Option;
use tokio::signal;
use tracing::info;

mod config;
mod server;
mod kubernetes;

use crate::config::init_config;
use crate::kubernetes::controller::Controller; // or K8sController

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let config: Arc<config::QubitConfig> = init_config();
    info!("Config loaded: {:?}", config);

    // spawn server as async task
    let server_cfg = config.clone();
    let mut server_handle = tokio::spawn(async move {
        // adapt new_http_server to return an object with async serve().await
        let server: Box<dyn server::Server> = server::new_http_server(server_cfg);
        if let Err(e) = server.do_serve().await {
            log::error!("server error: {:?}", e);
            panic!("error while starting server: {:?}", e);
        }

    });

    // spawn controller as async task
    let ctrl_cfg = config.clone();
    let mut controller_handle = tokio::spawn(async move {
        let ctrl = Controller::new(ctrl_cfg, None);
        if let Err(e) = ctrl.start_informers().await {
            log::error!("controller error: {}", e);
        }
    });
    
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("shutdown signal received");
        }
        res = &mut server_handle => {
            info!("server task finished: {:?}", res);
        }
        res = &mut controller_handle => {
            info!("controller task finished: {:?}", res);
        }
    }

    // Wait for tasks to exit cleanly (or force abort)
    let _ = server_handle.abort();
    let _ = controller_handle.abort();

    Ok(())
}
