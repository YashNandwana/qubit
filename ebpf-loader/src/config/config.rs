use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, OnceLock};

const CONFIG_FILE_PATH: &str = "/workspace/ebpf-loader/config.yaml";

#[derive(Debug, Serialize, Deserialize)]
pub struct EbpfLoaderConfig {
    pub qubit_core: QubitCoreConfig,
    pub perf_array_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QubitCoreConfig {
    pub host: String,
    pub port: u16,
}

static CONFIG: OnceLock<Arc<EbpfLoaderConfig>> = OnceLock::new();

pub fn init_config() -> Arc<EbpfLoaderConfig> {
    CONFIG
        .get_or_init(|| {
            let config_str =
                fs::read_to_string(CONFIG_FILE_PATH).expect("Failed to read config file");
            let parsed: EbpfLoaderConfig =
                serde_yaml::from_str(&config_str).expect("Failed to parse config file");
            Arc::new(parsed)
        })
        .clone()
}
