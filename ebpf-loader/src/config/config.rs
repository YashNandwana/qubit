use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::sync::{Arc, OnceLock};

fn get_config_path() -> String {
    env::var("CONFIG_PATH").unwrap_or_else(|_| "/app/config.yaml".to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EbpfLoaderConfig {
    pub qubit_core: QubitCoreConfig,
    pub perf_array_name: String,
    #[serde(default = "default_ebpf_path")]
    pub ebpf_path: String,
}

fn default_ebpf_path() -> String {
    "/workspace/ebpf/target/bpfel-unknown-none/release/ebpf".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QubitCoreConfig {
    pub host: String,
    pub grpc_port: u16,
}

static CONFIG: OnceLock<Arc<EbpfLoaderConfig>> = OnceLock::new();

pub fn init_config() -> Arc<EbpfLoaderConfig> {
    CONFIG
        .get_or_init(|| {
            let config_path = get_config_path();
            let config_str = fs::read_to_string(&config_path)
                .expect(&format!("Failed to read config file at {}", config_path));
            let parsed: EbpfLoaderConfig =
                serde_yaml::from_str(&config_str).expect("Failed to parse config file");
            Arc::new(parsed)
        })
        .clone()
}
