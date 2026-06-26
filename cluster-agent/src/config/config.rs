use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::sync::{Arc, OnceLock};

fn get_config_path() -> String {
    env::var("CONFIG_PATH").unwrap_or_else(|_| "/app/config.yaml".to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterAgentConfig {
    pub qubit_core: QubitCoreConfig,
    pub kubernetes: KubernetesConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QubitCoreConfig {
    pub host: String,
    pub grpc_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct KubernetesConfig {
    pub namespace: String,
}

impl Default for KubernetesConfig {
    fn default() -> Self {
        Self {
            namespace: String::new(),
        }
    }
}

static CONFIG: OnceLock<Arc<ClusterAgentConfig>> = OnceLock::new();

pub fn init_config() -> Arc<ClusterAgentConfig> {
    CONFIG
        .get_or_init(|| {
            let config_path = get_config_path();
            let config_str = fs::read_to_string(&config_path)
                .unwrap_or_else(|_| panic!("Failed to read config file at {}", config_path));
            let parsed: ClusterAgentConfig =
                serde_yaml::from_str(&config_str).expect("Failed to parse config file");
            Arc::new(parsed)
        })
        .clone()
}
