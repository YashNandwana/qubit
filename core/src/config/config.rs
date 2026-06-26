use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, OnceLock};

const CONFIG_FILE_PATH: &str = "config.yaml";

#[derive(Debug, Serialize, Deserialize)]
pub struct QubitConfig {
    pub app: AppConfig,
    pub kubernetes: KubernetesConfig,
    pub db: DbConfig,
    pub envoy: EnvoyConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvoyConfig {
    pub admin_api: String,
    pub refresh_interval_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub http_port: u16,
    pub grpc_port: u16,
    pub upstream: String,
    pub ebpf_bulk_insertion_max_size: u16,
    pub ebpf_flush_interval_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub in_cluster: bool,
    pub namespace: String,
    pub leader_election: LeaderElectionConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaderElectionConfig {
    pub enabled: bool,
    pub lease_duration: String,
    pub renew_deadline: String,
    pub retry_period: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub table: TableConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TableConfig {
    pub ebpf_network_events: String,
    pub k8s_resource_events: String,
}

static CONFIG: OnceLock<Arc<QubitConfig>> = OnceLock::new();

pub fn init_config() -> Arc<QubitConfig> {
    CONFIG
        .get_or_init(|| {
            let config_str =
                fs::read_to_string(CONFIG_FILE_PATH).expect("Failed to read config file");
            let parsed: QubitConfig =
                serde_yaml::from_str(&config_str).expect("Failed to parse config file");
            Arc::new(parsed)
        })
        .clone()
}
