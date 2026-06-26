use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::server::handler;
use crate::topology::Topology;
use anyhow::Result;
use axum::extract::FromRef;
use axum::{Router, routing::get};
use log::info;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;

// ── Shared application state ──────────────────────────────────────────────────
//
// Axum's `FromRef` pattern is the idiomatic way to share multiple pieces of
// state across handlers without bundling everything into a single giant struct
// that every handler must accept.
//
// Think of it like Spring's multi-bean injection, but resolved at compile time
// via traits rather than at runtime via reflection.  Each handler extractor
// declares exactly what it needs:
//
//   topology handler  → State(topology): State<Arc<RwLock<Topology>>>
//   k8s_events handler → State(db): State<Arc<DAO>>
//
// Axum calls the matching `FromRef` impl to pull the right piece out of the
// root `AppState`.  No handler needs to know about the fields it doesn't use.

#[derive(Clone)]
pub struct AppState {
    pub topology: Arc<RwLock<Topology>>,
    pub db: Arc<DAO>,
}

impl FromRef<AppState> for Arc<RwLock<Topology>> {
    fn from_ref(state: &AppState) -> Self {
        state.topology.clone()
    }
}

impl FromRef<AppState> for Arc<DAO> {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

// ── HTTP server ───────────────────────────────────────────────────────────────

pub struct HttpServer {
    app_port: u16,
    app_host: String,
    state: AppState,
}

impl HttpServer {
    pub fn new(config: Arc<QubitConfig>, db: Arc<DAO>, topology: Arc<RwLock<Topology>>) -> Self {
        Self {
            app_host: String::from("0.0.0.0"),
            app_port: config.app.http_port,
            state: AppState { topology, db },
        }
    }

    pub async fn do_serve(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.app_host, self.app_port)
            .parse()
            .expect("invalid listen addr");

        info!("Starting HTTP server on {}", addr);

        let app = Router::new()
            .route("/ping", get(handler::health))
            .route("/api/topology", get(handler::topology))
            .route("/api/topology/subgraph", get(handler::topology_subgraph))
            .route("/api/k8s-events", get(handler::k8s_events))
            .route("/api/network-events", get(handler::network_events))
            .with_state(self.state.clone())
            .layer(CorsLayer::permissive());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }
}
