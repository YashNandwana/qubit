use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct McpConfig {
    pub qubit_core: CoreConfig,
    pub clickhouse: ClickHouseConfig,
}

#[derive(Debug, Deserialize)]
pub struct CoreConfig {
    pub grpc_address: String,
}

#[derive(Debug, Deserialize)]
pub struct ClickHouseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ebpf_table: String,
    pub k8s_table: String,
}

pub fn load_config() -> anyhow::Result<McpConfig> {
    // Look for config relative to the binary, then fall back to current dir.
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("config.yaml"))),
        Some(std::path::PathBuf::from("config.yaml")),
        Some(std::path::PathBuf::from("mcp-server/config.yaml")),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.exists() {
            let content = std::fs::read_to_string(candidate)?;
            let config: McpConfig = serde_yaml::from_str(&content)?;
            return Ok(config);
        }
    }

    anyhow::bail!("config.yaml not found")
}
