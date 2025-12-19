use axum::{routing::get, Router, Json};
use serde_json::{json, Value};
use log::info;
use std::net::SocketAddr;
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::config::QubitConfig;

#[async_trait]
pub trait Server: Send + Sync {
    async fn do_serve(&self) -> Result<()>;
    fn register(&self) -> Router;
}

#[allow(unused)]
pub struct HttpServer {
    app_port: u16,
    app_host: String,
    config:   Arc<QubitConfig>,
}

pub fn new_http_server(cfg: Arc<QubitConfig>) -> Box<dyn Server + Send + Sync> {
    Box::new(HttpServer {
        app_host: String::from("127.0.0.1"),
        app_port: cfg.app.http_port,
        config:   cfg,
    })
}

#[async_trait]
impl Server for HttpServer {
    async fn do_serve(&self) -> Result<()> {
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
        Router::new().route("/ping", get(health))
    }
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "pong" }))
}
