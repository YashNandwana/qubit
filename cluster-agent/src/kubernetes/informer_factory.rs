use std::sync::Arc;

use k8s_openapi::api::{
    apps::v1::{Deployment, ReplicaSet},
    autoscaling::v2::HorizontalPodAutoscaler,
    core::v1::{ConfigMap, Event, Pod, Service},
    networking::v1::Ingress,
};

use super::configmap_handler::ConfigMapHandler;
use super::crd_types::{ExternalSecret, HTTPProxy, Rollout, VirtualService};
use super::event_handler::K8sEventHandler;
use super::generic_handler::GenericHandler;
use super::informer::{Informer, InformerGeneric, InformerType};
use super::node_informer::NodeInformer;
use super::pod_handler::PodHandler;
use super::service_handler::ServiceHandler;
use super::service_registry::ServiceRegistry;
use crate::config::ClusterAgentConfig;
use crate::service::ClusterAggregator;

pub struct InformerFactory {
    config: Arc<ClusterAgentConfig>,
    aggregator: Arc<ClusterAggregator>,
    // Shared between ServiceHandler (writer) and PodHandler (reader)
    registry: Arc<ServiceRegistry>,
}

impl InformerFactory {
    pub fn new(config: Arc<ClusterAgentConfig>, aggregator: Arc<ClusterAggregator>) -> Self {
        Self {
            config,
            aggregator,
            registry: Arc::new(ServiceRegistry::new()),
        }
    }

    // ── Existing handlers with specific logic ────────────────────────

    pub fn create_configmap_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<ConfigMap, ConfigMapHandler>::new(
            self.config.clone(),
            InformerType::ConfigMap,
            ConfigMapHandler::new(self.aggregator.clone()),
        )
    }

    pub fn create_service_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Service, ServiceHandler>::new(
            self.config.clone(),
            InformerType::Service,
            ServiceHandler::new(self.aggregator.clone(), self.registry.clone()),
        )
    }

    pub fn create_pod_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Pod, PodHandler>::new(
            self.config.clone(),
            InformerType::Pod,
            PodHandler::new(self.aggregator.clone(), self.registry.clone()),
        )
    }

    // ── Native K8s resources using GenericHandler ────────────────────

    pub fn create_deployment_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Deployment, GenericHandler>::new(
            self.config.clone(),
            InformerType::Deployment,
            GenericHandler::new(self.aggregator.clone(), "Deployment"),
        )
    }

    pub fn create_replicaset_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<ReplicaSet, GenericHandler>::new(
            self.config.clone(),
            InformerType::ReplicaSet,
            GenericHandler::new(self.aggregator.clone(), "ReplicaSet"),
        )
    }

    pub fn create_ingress_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Ingress, GenericHandler>::new(
            self.config.clone(),
            InformerType::Ingress,
            GenericHandler::new(self.aggregator.clone(), "Ingress"),
        )
    }

    /// K8s Event gets a dedicated handler that extracts reason/message/type
    /// rather than just metadata — these are critical for AI debugging context.
    pub fn create_event_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Event, K8sEventHandler>::new(
            self.config.clone(),
            InformerType::Event,
            K8sEventHandler::new(self.aggregator.clone()),
        )
    }

    pub fn create_hpa_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<HorizontalPodAutoscaler, GenericHandler>::new(
            self.config.clone(),
            InformerType::Hpa,
            GenericHandler::new(self.aggregator.clone(), "HPA"),
        )
    }

    /// Node is cluster-scoped (not namespace-scoped), so it uses a dedicated
    /// NodeInformer instead of InformerGeneric which constrains to
    /// NamespaceResourceScope.
    pub fn create_node_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        Arc::new(NodeInformer::new(GenericHandler::new(
            self.aggregator.clone(),
            "Node",
        )))
    }

    // ── CRDs using GenericHandler ───────────────────────────────────
    // These will gracefully error if the CRD is not installed in the cluster.

    pub fn create_rollout_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<Rollout, GenericHandler>::new(
            self.config.clone(),
            InformerType::Rollout,
            GenericHandler::new(self.aggregator.clone(), "Rollout"),
        )
    }

    pub fn create_external_secret_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<ExternalSecret, GenericHandler>::new(
            self.config.clone(),
            InformerType::ExternalSecret,
            GenericHandler::new(self.aggregator.clone(), "ExternalSecret"),
        )
    }

    pub fn create_http_proxy_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<HTTPProxy, GenericHandler>::new(
            self.config.clone(),
            InformerType::HttpProxy,
            GenericHandler::new(self.aggregator.clone(), "HTTPProxy"),
        )
    }

    pub fn create_virtual_service_informer(&self) -> Arc<dyn Informer + Send + Sync> {
        InformerGeneric::<VirtualService, GenericHandler>::new(
            self.config.clone(),
            InformerType::VirtualService,
            GenericHandler::new(self.aggregator.clone(), "VirtualService"),
        )
    }
}
