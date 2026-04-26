use std::sync::Arc;

use tonic::transport::Channel;

use crate::config::ClusterAgentConfig;
use std::collections::HashMap;

use crate::proto::qubit::event_ingestion_client::EventIngestionClient;
use crate::proto::qubit::{
    ConfigMapEventRequest, K8sEventType, K8sResourceEventRequest, PodEventRequest,
    ServiceEventRequest, ServicePodEntry, ServicePodMapRequest,
};

pub struct ClusterAggregator {
    client: EventIngestionClient<Channel>,
}

impl ClusterAggregator {
    pub fn new(config: Arc<ClusterAgentConfig>) -> Self {
        let endpoint = format!(
            "http://{}:{}",
            config.qubit_core.host, config.qubit_core.grpc_port
        );
        let channel = Channel::from_shared(endpoint)
            .expect("invalid gRPC endpoint")
            .connect_lazy();
        Self {
            client: EventIngestionClient::new(channel),
        }
    }

    pub async fn send_pod_applied(
        &self,
        pod_ip: String,
        namespace: String,
        service_name: String,
        service_type: String,
    ) -> Result<(), tonic::Status> {
        self.client.clone().send_pod_event(PodEventRequest {
            pod_ip,
            namespace,
            service_name,
            service_type,
            event_type: K8sEventType::Applied as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_pod_deleted(&self, pod_ip: String) -> Result<(), tonic::Status> {
        self.client.clone().send_pod_event(PodEventRequest {
            pod_ip,
            namespace: String::new(),
            service_name: String::new(),
            service_type: String::new(),
            event_type: K8sEventType::Deleted as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_service_applied(
        &self,
        name: String,
        namespace: String,
        service_type: String,
        cluster_ip: String,
    ) -> Result<(), tonic::Status> {
        self.client.clone().send_service_event(ServiceEventRequest {
            name,
            namespace,
            service_type,
            cluster_ip,
            event_type: K8sEventType::Applied as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_service_deleted(&self, name: String, namespace: String) -> Result<(), tonic::Status> {
        self.client.clone().send_service_event(ServiceEventRequest {
            name,
            namespace,
            service_type: String::new(),
            cluster_ip: String::new(),
            event_type: K8sEventType::Deleted as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_configmap_applied(&self, name: String, namespace: String) -> Result<(), tonic::Status> {
        self.client.clone().send_config_map_event(ConfigMapEventRequest {
            name,
            namespace,
            event_type: K8sEventType::Applied as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_configmap_deleted(&self, name: String, namespace: String) -> Result<(), tonic::Status> {
        self.client.clone().send_config_map_event(ConfigMapEventRequest {
            name,
            namespace,
            event_type: K8sEventType::Deleted as i32,
        }).await?;
        Ok(())
    }

    pub async fn send_k8s_resource_event(
        &self,
        resource_type: String,
        name: String,
        namespace: String,
        event_type: K8sEventType,
        labels: HashMap<String, String>,
        resource_data: String,
    ) -> Result<(), tonic::Status> {
        self.client
            .clone()
            .send_k8s_resource_event(K8sResourceEventRequest {
                resource_type,
                name,
                namespace,
                event_type: event_type as i32,
                labels,
                resource_data,
            })
            .await?;
        Ok(())
    }

    pub async fn send_service_pod_map(
        &self,
        entries: Vec<(String, String, String, Vec<String>)>,
    ) -> Result<(), tonic::Status> {
        self.client.clone().send_service_pod_map(ServicePodMapRequest {
            entries: entries
                .into_iter()
                .map(|(service_name, namespace, service_type, pod_ips)| ServicePodEntry {
                    service_name,
                    namespace,
                    service_type,
                    pod_ips,
                })
                .collect(),
        }).await?;
        Ok(())
    }
}
