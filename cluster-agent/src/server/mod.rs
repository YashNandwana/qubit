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

    // Helper macro to reduce boilerplate — each informer gets a tokio task
    macro_rules! spawn_informer {
        ($name:expr, $informer:expr, $client:expr) => {{
            let informer = $informer.clone();
            let client = $client.clone();
            tokio::spawn(async move {
                if let Err(e) = informer.start(client).await {
                    log::error!("{} informer failed: {}", $name, e);
                }
            })
        }};
    }

    // Existing informers
    let mut cm_handle = spawn_informer!("ConfigMap", informers.configmap, client);
    let mut svc_handle = spawn_informer!("Service", informers.service, client);
    let mut pod_handle = spawn_informer!("Pod", informers.pod, client);

    // Native K8s resources
    let mut deploy_handle = spawn_informer!("Deployment", informers.deployment, client);
    let mut rs_handle = spawn_informer!("ReplicaSet", informers.replicaset, client);
    let mut ingress_handle = spawn_informer!("Ingress", informers.ingress, client);
    let mut event_handle = spawn_informer!("Event", informers.event, client);
    let mut hpa_handle = spawn_informer!("HPA", informers.hpa, client);
    let mut node_handle = spawn_informer!("Node", informers.node, client);

    // CRDs — these will log errors if CRDs aren't installed (expected in dev)
    let mut rollout_handle = spawn_informer!("Rollout", informers.rollout, client);
    let mut es_handle = spawn_informer!("ExternalSecret", informers.external_secret, client);
    let mut hp_handle = spawn_informer!("HTTPProxy", informers.http_proxy, client);
    let mut vs_handle = spawn_informer!("VirtualService", informers.virtual_service, client);

    log::info!("Started all cluster informers");

    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Shutdown signal received");
        }
        // Existing
        res = &mut cm_handle => log::error!("ConfigMap informer task finished: {:?}", res),
        res = &mut svc_handle => log::error!("Service informer task finished: {:?}", res),
        res = &mut pod_handle => log::error!("Pod informer task finished: {:?}", res),
        // Native K8s
        res = &mut deploy_handle => log::error!("Deployment informer task finished: {:?}", res),
        res = &mut rs_handle => log::error!("ReplicaSet informer task finished: {:?}", res),
        res = &mut ingress_handle => log::error!("Ingress informer task finished: {:?}", res),
        res = &mut event_handle => log::error!("Event informer task finished: {:?}", res),
        res = &mut hpa_handle => log::error!("HPA informer task finished: {:?}", res),
        res = &mut node_handle => log::error!("Node informer task finished: {:?}", res),
        // CRDs
        res = &mut rollout_handle => log::error!("Rollout informer task finished: {:?}", res),
        res = &mut es_handle => log::error!("ExternalSecret informer task finished: {:?}", res),
        res = &mut hp_handle => log::error!("HTTPProxy informer task finished: {:?}", res),
        res = &mut vs_handle => log::error!("VirtualService informer task finished: {:?}", res),
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
