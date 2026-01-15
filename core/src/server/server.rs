use crate::config::QubitConfig;
use crate::server::handler;
use anyhow::Result;
use axum::routing::post;
use axum::{Router, routing::get};
use log::info;
use std::net::SocketAddr;
use std::sync::Arc;

#[allow(unused)]
pub struct HttpServer {
    app_port: u16,
    app_host: String,
    config: Arc<QubitConfig>,
}

impl HttpServer {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        Self {
            app_host: String::from("127.0.0.1"),
            app_port: config.app.http_port,
            config,
        }
    }

    pub async fn do_serve(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.app_host, self.app_port)
            .parse()
            .expect("invalid listen addr");

        info!("Starting HTTP server on {}", addr);
        let app = self.register();

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }

    fn register(&self) -> Router {
        let state = handler::AppState::new(self.config.clone());

        Router::new()
            .route("/ping", get(handler::health))
            .route(
                "/aggregate/ebpf/network",
                post(handler::aggregate_ebpf_network),
            )
            .with_state(state)
    }
}
