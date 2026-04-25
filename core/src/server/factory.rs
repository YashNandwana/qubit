use std::sync::{Arc, RwLock};

use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::topology::Topology;

use super::grpc::GrpcServer;
use super::http::HttpServer;
use super::query::QueryServer;

pub struct ServerFactory {
    config: Arc<QubitConfig>,
    db: Arc<DAO>,
    topology: Arc<RwLock<Topology>>,
}

impl ServerFactory {
    pub fn new(config: Arc<QubitConfig>, db: Arc<DAO>, topology: Arc<RwLock<Topology>>) -> Self {
        Self { config, db, topology }
    }

    pub fn http(&self) -> HttpServer {
        HttpServer::new(self.config.clone(), self.db.clone(), self.topology.clone())
    }

    pub fn grpc(&self) -> GrpcServer {
        GrpcServer::new(self.config.clone(), self.db.clone(), self.topology.clone())
    }

    pub fn query(&self) -> QueryServer {
        QueryServer::new(self.topology.clone())
    }
}
