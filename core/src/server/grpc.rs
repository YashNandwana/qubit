use std::sync::{Arc, RwLock};

use tonic::{Request, Response, Status};

use crate::aggregator::{EbpfAggregator, K8sAggregator};
use crate::config::QubitConfig;
use crate::dao::DAO;
use crate::model::{EbpfNetworkEventInput};
use crate::topology::Topology;

use super::query::QueryServer;
use super::qubit::event_ingestion_server::{EventIngestion, EventIngestionServer};
use super::qubit::qubit_query_server::QubitQueryServer;
use super::qubit::{
    ConfigMapEventRequest, ConfigMapEventResponse,
    EbpfNetworkEventRequest, EbpfNetworkEventResponse,
    K8sEventType,
    K8sResourceEventRequest, K8sResourceEventResponse,
    PodEventRequest, PodEventResponse,
    ServiceEventRequest, ServiceEventResponse,
    ServicePodMapRequest, ServicePodMapResponse,
    FILE_DESCRIPTOR_SET,
};

pub struct GrpcServer {
    config: Arc<QubitConfig>,
    ebpf_aggregator: Arc<EbpfAggregator>,
    k8s_aggregator: Arc<K8sAggregator>,
}

impl GrpcServer {
    pub fn new(config: Arc<QubitConfig>, db: Arc<DAO>, topology: Arc<RwLock<Topology>>) -> Self {
        let k8s_aggregator = Arc::new(K8sAggregator::new(topology.clone(), db.clone()));
        let pod_cache = k8s_aggregator.pod_cache();
        let ebpf_aggregator = Arc::new(EbpfAggregator::new(config.clone(), db, topology, pod_cache));
        Self { config, ebpf_aggregator, k8s_aggregator }
    }

    pub async fn do_serve(self, query_server: QueryServer) -> Result<(), String> {
        self.ebpf_aggregator.start_flush_timer(self.config.app.ebpf_flush_interval_secs);

        let addr = format!("0.0.0.0:{}", self.config.app.grpc_port)
            .parse()
            .map_err(|e: std::net::AddrParseError| e.to_string())?;

        log::info!("gRPC server listening on {}", addr);

        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()
            .map_err(|e| e.to_string())?;

        tonic::transport::Server::builder()
            .add_service(reflection)
            .add_service(EventIngestionServer::new(self))
            .add_service(QubitQueryServer::new(query_server))
            .serve(addr)
            .await
            .map_err(|e| e.to_string())
    }
}

#[tonic::async_trait]
impl EventIngestion for GrpcServer {
    async fn send_ebpf_network_event(
        &self,
        request: Request<EbpfNetworkEventRequest>,
    ) -> Result<Response<EbpfNetworkEventResponse>, Status> {
        let req = request.into_inner();
        let input = EbpfNetworkEventInput {
            timestamp_ns: req.timestamp_ns,
            src_ip: req.src_ip,
            dst_ip: req.dst_ip,
            src_port: req.src_port as u16,
            dst_port: req.dst_port as u16,
            method: req.method,
            path: req.path,
            host: req.host,
        };

        let aggregator = self.ebpf_aggregator.clone();
        tokio::spawn(async move {
            let _ = aggregator.record_ebpf_event(input).await;
        });

        Ok(Response::new(EbpfNetworkEventResponse {
            success: true,
            message: "Event received".to_string(),
        }))
    }

    async fn send_pod_event(
        &self,
        request: Request<PodEventRequest>,
    ) -> Result<Response<PodEventResponse>, Status> {
        let req = request.into_inner();
        match K8sEventType::try_from(req.event_type) {
            Ok(K8sEventType::Applied) => {
                log::info!(
                    "Pod applied: {} -> {} (service: {})",
                    req.pod_ip, req.namespace, req.service_name
                );
                self.k8s_aggregator.record_pod_applied(
                    &req.pod_ip,
                    &req.namespace,
                    &req.service_name,
                    &req.service_type,
                );
            }
            Ok(K8sEventType::Deleted) => {
                log::info!("Pod deleted: {}", req.pod_ip);
                self.k8s_aggregator.record_pod_deleted(&req.pod_ip);
            }
            Err(_) => return Err(Status::invalid_argument("unknown event type")),
        }
        Ok(Response::new(PodEventResponse { success: true, message: "ok".to_string() }))
    }

    async fn send_service_event(
        &self,
        request: Request<ServiceEventRequest>,
    ) -> Result<Response<ServiceEventResponse>, Status> {
        let req = request.into_inner();
        match K8sEventType::try_from(req.event_type) {
            Ok(K8sEventType::Applied) => {
                log::info!("Service applied: {}/{} ({}, {})", req.namespace, req.name, req.service_type, req.cluster_ip);
                self.k8s_aggregator.record_service_applied(&req.name, &req.namespace, &req.service_type, &req.cluster_ip);
            }
            Ok(K8sEventType::Deleted) => {
                log::info!("Service deleted: {}/{}", req.namespace, req.name);
                self.k8s_aggregator.record_service_deleted(&req.name, &req.namespace);
            }
            Err(_) => return Err(Status::invalid_argument("unknown event type")),
        }
        Ok(Response::new(ServiceEventResponse { success: true, message: "ok".to_string() }))
    }

    async fn send_config_map_event(
        &self,
        request: Request<ConfigMapEventRequest>,
    ) -> Result<Response<ConfigMapEventResponse>, Status> {
        let req = request.into_inner();
        match K8sEventType::try_from(req.event_type) {
            Ok(K8sEventType::Applied) => log::info!("ConfigMap applied: {}/{}", req.namespace, req.name),
            Ok(K8sEventType::Deleted) => log::info!("ConfigMap deleted: {}/{}", req.namespace, req.name),
            Err(_) => return Err(Status::invalid_argument("unknown event type")),
        }
        Ok(Response::new(ConfigMapEventResponse { success: true, message: "ok".to_string() }))
    }

    async fn send_service_pod_map(
        &self,
        request: Request<ServicePodMapRequest>,
    ) -> Result<Response<ServicePodMapResponse>, Status> {
        let entries = request.into_inner().entries;
        let count = entries.len();
        for entry in entries {
            for pod_ip in &entry.pod_ips {
                self.k8s_aggregator.record_pod_applied(
                    pod_ip,
                    &entry.namespace,
                    &entry.service_name,
                    &entry.service_type,
                );
            }
        }
        log::info!("Initial pod-service map applied: {} service entries", count);
        Ok(Response::new(ServicePodMapResponse {
            success: true,
            message: format!("Applied {} service entries", count),
        }))
    }

    async fn send_k8s_resource_event(
        &self,
        request: Request<K8sResourceEventRequest>,
    ) -> Result<Response<K8sResourceEventResponse>, Status> {
        let req = request.into_inner();
        let event_type_str = match K8sEventType::try_from(req.event_type) {
            Ok(K8sEventType::Applied) => "Applied",
            Ok(K8sEventType::Deleted) => "Deleted",
            Err(_) => return Err(Status::invalid_argument("unknown event type")),
        };

        log::debug!(
            "{} {}: {}/{}",
            req.resource_type,
            event_type_str,
            req.namespace,
            req.name
        );

        self.k8s_aggregator.record_k8s_resource_event(
            &req.resource_type,
            &req.name,
            &req.namespace,
            event_type_str,
            &req.labels,
            &req.resource_data,
        );

        Ok(Response::new(K8sResourceEventResponse {
            success: true,
            message: "ok".to_string(),
        }))
    }
}
