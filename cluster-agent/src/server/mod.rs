use std::sync::Arc;

use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{Api, Client};
use tokio::signal;

use crate::config::ClusterAgentConfig;
use crate::kubernetes::Controller;
use crate::service::ClusterAggregator;

pub async fn run(
    config: Arc<ClusterAgentConfig>,
    client: Client,
    aggregator: Arc<ClusterAggregator>,
) -> anyhow::Result<()> {
    if let Err(e) = send_initial_pod_service_map(client.clone(), aggregator.clone()).await {
        log::warn!("Initial pod-service map failed (core may not be ready yet): {}", e);
    }

    let informers = Controller::new(config, aggregator).create_informers();

    let cm_informer = informers.configmap.clone();
    let cm_client = client.clone();
    let mut cm_handle = tokio::spawn(async move {
        if let Err(e) = cm_informer.start(cm_client).await {
            log::error!("ConfigMap informer failed: {}", e);
        }
    });

    let svc_informer = informers.service.clone();
    let svc_client = client.clone();
    let mut svc_handle = tokio::spawn(async move {
        if let Err(e) = svc_informer.start(svc_client).await {
            log::error!("Service informer failed: {}", e);
        }
    });

    let pod_informer = informers.pod.clone();
    let pod_client = client.clone();
    let mut pod_handle = tokio::spawn(async move {
        if let Err(e) = pod_informer.start(pod_client).await {
            log::error!("Pod informer failed: {}", e);
        }
    });

    log::info!("Started all cluster informers");

    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Shutdown signal received");
        }
        res = &mut cm_handle => {
            log::error!("ConfigMap informer task finished: {:?}", res);
        }
        res = &mut svc_handle => {
            log::error!("Service informer task finished: {:?}", res);
        }
        res = &mut pod_handle => {
            log::error!("Pod informer task finished: {:?}", res);
        }
    }

    Ok(())
}

async fn send_initial_pod_service_map(
    client: Client,
    aggregator: Arc<ClusterAggregator>,
) -> anyhow::Result<()> {
    let services: Api<Service> = Api::all(client.clone());
    let pods: Api<Pod> = Api::all(client.clone());

    let svc_list = services.list(&Default::default()).await?;
    let pod_list = pods.list(&Default::default()).await?;

    let mut entries: Vec<(String, String, String, Vec<String>)> = Vec::new();

    for svc in &svc_list.items {
        let name = match svc.metadata.name.as_deref() {
            Some(n) => n,
            None => continue,
        };
        let namespace = svc.metadata.namespace.clone().unwrap_or_default();
        let spec = match svc.spec.as_ref() {
            Some(s) => s,
            None => continue,
        };
        let selector = spec.selector.clone().unwrap_or_default();
        if selector.is_empty() {
            continue;
        }
        let service_type = spec.type_.clone().unwrap_or_else(|| "ClusterIP".to_string());

        let pod_ips: Vec<String> = pod_list
            .items
            .iter()
            .filter(|pod| {
                let labels = pod.metadata.labels.as_ref();
                selector.iter().all(|(k, v)| labels.and_then(|l| l.get(k)) == Some(v))
            })
            .filter_map(|pod| pod.status.as_ref()?.pod_ip.clone())
            .collect();

        if !pod_ips.is_empty() {
            entries.push((name.to_string(), namespace, service_type, pod_ips));
        }
    }

    log::info!("Sending initial pod-service map: {} service entries", entries.len());
    aggregator.send_service_pod_map(entries).await?;
    Ok(())
}
