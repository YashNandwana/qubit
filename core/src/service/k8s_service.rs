use crate::config::QubitConfig;
use crate::kubernetes::controller::Controller;
use std::sync::Arc;

use kube::Client;

pub trait K8sService {
    async fn informer_service(&self) -> Result<(), String>;
}

pub struct K8sServiceImpl {
    config: Arc<QubitConfig>,
}

impl K8sServiceImpl {
    pub fn new(config: Arc<QubitConfig>) -> Self {
        Self { config }
    }

    async fn await_informers(&self,
        cm_handle: tokio::task::JoinHandle<()>,
        svc_handle: tokio::task::JoinHandle<()>) -> Result<(), String> {
        tokio::select! {
            res = cm_handle => {
                if let Err(e) = res {
                    return Err(format!("ConfigMap informer task panicked: {}", e));
                }
            }
            res = svc_handle => {
                if let Err(e) = res {
                    return Err(format!("Service informer task panicked: {}", e));
                }
            }
        }
        Ok(())
    }
}

impl K8sService for K8sServiceImpl {
    async fn informer_service(&self) -> Result<(), String> {
        let controller = Controller::new(self.config.clone());
        let informers = controller.create_informers();

        let kube_cfg = match kube::Config::incluster() {
            Ok(cfg) => cfg,
            Err(_) => kube::Config::infer()
                .await
                .map_err(|e| format!("failed to infer kube config: {}", e))?,
        };
        let client = Client::try_from(kube_cfg)
            .map_err(|e| format!("failed to create kube client: {}", e))?;

        // Spawn informers as concurrent tasks
        // TODO: avoid multiple calls, Composite Pattern maybe?
        let cm_informer = informers.configmap.clone();
        let cm_client = client.clone();
        let cm_handle = tokio::spawn(async move {
            if let Err(e) = cm_informer.start(cm_client).await {
                log::error!("ConfigMap informer failed: {}", e);
            }
        });

        let svc_informer = informers.service.clone();
        let svc_client = client.clone();
        let svc_handle = tokio::spawn(async move {
            if let Err(e) = svc_informer.start(svc_client).await {
                log::error!("Service informer failed: {}", e);
            }
        });

        log::info!("Started all K8s informers");

        self.await_informers(cm_handle, svc_handle).await?;

        Ok(())
    }
}
