use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::{ConfigMap, Pod, Service};
use kube::{Api, Client};
use tokio::signal;

use crate::config::ClusterAgentConfig;
use crate::kubernetes::{parse_envoy_routes, Controller};
use crate::service::ClusterAggregator;

pub async fn run(
    config: Arc<ClusterAgentConfig>,
    client: Client,
    aggregator: Arc<ClusterAggregator>,
) -> anyhow::Result<()> {
    if let Err(e) = send_initial_pod_service_map(client.clone(), aggregator.clone()).await {
        log::warn!(
            "Initial pod-service map failed (core may not be ready yet): {}",
            e
        );
    }

    // Re-send the full pod map and envoy routes every 30s. This covers two cases:
    // 1. Startup race — test pods weren't Ready yet when the initial map was sent
    // 2. Core restart — cluster-agent is in watch mode and won't replay existing resources
    {
        let client = client.clone();
        let aggregator = aggregator.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(30));
            ticker.tick().await; // first tick fires immediately — skip it, already sent above
            loop {
                ticker.tick().await;
                if let Err(e) =
                    send_initial_pod_service_map(client.clone(), aggregator.clone()).await
                {
                    log::warn!("Pod-service map resync failed: {}", e);
                }
                if let Err(e) =
                    send_envoy_routes_from_configmaps(client.clone(), aggregator.clone()).await
                {
                    log::warn!("Envoy routes resync failed: {}", e);
                }
            }
        });
    }

    // Send envoy routes on startup (cluster-agent already has ConfigMaps from informer cache,
    // but the initial configmap handler fires before core is ready — resync covers that gap).
    if let Err(e) = send_envoy_routes_from_configmaps(client.clone(), aggregator.clone()).await {
        log::warn!(
            "Initial envoy routes send failed (core may not be ready yet): {}",
            e
        );
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

    // CRD informers are optional — if the CRD isn't installed the watcher
    // returns 404 immediately. Dropping the handle detaches the task so its
    // completion doesn't trigger the select! below and kill the whole server.
    drop(spawn_informer!("Rollout", informers.rollout, client));
    drop(spawn_informer!(
        "ExternalSecret",
        informers.external_secret,
        client
    ));
    drop(spawn_informer!("HTTPProxy", informers.http_proxy, client));
    drop(spawn_informer!(
        "VirtualService",
        informers.virtual_service,
        client
    ));

    log::info!("Started all cluster informers");

    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Shutdown signal received");
        }
        res = &mut cm_handle => log::error!("ConfigMap informer task finished: {:?}", res),
        res = &mut svc_handle => log::error!("Service informer task finished: {:?}", res),
        res = &mut pod_handle => log::error!("Pod informer task finished: {:?}", res),
        res = &mut deploy_handle => log::error!("Deployment informer task finished: {:?}", res),
        res = &mut rs_handle => log::error!("ReplicaSet informer task finished: {:?}", res),
        res = &mut ingress_handle => log::error!("Ingress informer task finished: {:?}", res),
        res = &mut event_handle => log::error!("Event informer task finished: {:?}", res),
        res = &mut hpa_handle => log::error!("HPA informer task finished: {:?}", res),
        res = &mut node_handle => log::error!("Node informer task finished: {:?}", res),
    }

    Ok(())
}

async fn send_envoy_routes_from_configmaps(
    client: Client,
    aggregator: Arc<ClusterAggregator>,
) -> anyhow::Result<()> {
    let cms: Api<ConfigMap> = Api::all(client);
    let cm_list = cms.list(&Default::default()).await?;

    let mut all_routes = Vec::new();
    for cm in &cm_list.items {
        if let Some(envoy_yaml) = cm.data.as_ref().and_then(|d| d.get("envoy.yaml")) {
            let routes = parse_envoy_routes(envoy_yaml);
            log::info!(
                "Envoy ConfigMap {}/{}: {} routes parsed",
                cm.metadata.namespace.as_deref().unwrap_or("?"),
                cm.metadata.name.as_deref().unwrap_or("?"),
                routes.len()
            );
            all_routes.extend(routes);
        }
    }

    if !all_routes.is_empty() {
        log::info!("Sending {} envoy route entries to core", all_routes.len());
        aggregator.send_envoy_routes(all_routes).await?;
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
        let service_type = spec
            .type_
            .clone()
            .unwrap_or_else(|| "ClusterIP".to_string());

        let pod_ips: Vec<String> = pod_list
            .items
            .iter()
            .filter(|pod| {
                let labels = pod.metadata.labels.as_ref();
                selector
                    .iter()
                    .all(|(k, v)| labels.and_then(|l| l.get(k)) == Some(v))
            })
            .filter_map(|pod| pod.status.as_ref()?.pod_ip.clone())
            .collect();

        if !pod_ips.is_empty() {
            entries.push((name.to_string(), namespace, service_type, pod_ips));
        }
    }

    log::info!(
        "Sending initial pod-service map: {} service entries",
        entries.len()
    );
    aggregator.send_service_pod_map(entries).await?;
    Ok(())
}
