use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::server::handler;
use crate::topology::Topology;
use anyhow::Result;
use axum::{Router, routing::get};
use log::info;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

pub struct HttpServer {
    app_port: u16,
    app_host: String,
}

impl HttpServer {
    pub fn new(config: Arc<QubitConfig>, _db: Arc<DAO>, _topology: Arc<RwLock<Topology>>) -> Self {
        Self {
            app_host: String::from("0.0.0.0"),
            app_port: config.app.http_port,
        }
    }

    pub async fn do_serve(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.app_host, self.app_port)
            .parse()
            .expect("invalid listen addr");

        info!("Starting HTTP server on {}", addr);

        let app = Router::new().route("/ping", get(handler::health));

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }
}
