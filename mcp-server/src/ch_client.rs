use anyhow::Result;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use crate::config::ClickHouseConfig;

/// ClickHouse query client — read-only access to event tables.
#[derive(Clone)]
pub struct ChClient {
    client: clickhouse::Client,
    ebpf_table: String,
    k8s_table: String,
}

/// Row shape returned from ebpf_network_events.
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct EbpfRow {
    pub timestamp_ns: u64,
    pub src_service: String,
    pub src_namespace: String,
    pub dst_service: String,
    pub dst_namespace: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub method: String,
    pub path: String,
    pub host: String,
}

/// Row shape returned from k8s_resource_events.
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct K8sRow {
    pub event_time: u32,
    pub resource_type: String,
    pub name: String,
    pub namespace: String,
    pub event_type: String,
    pub labels: String,
    pub resource_data: String,
}

impl ChClient {
    pub fn new(config: &ClickHouseConfig) -> Self {
        let url = format!("http://{}:{}", config.host, config.port);
        let client = clickhouse::Client::default()
            .with_url(&url)
            .with_database(&config.database)
            .with_user(&config.user)
            .with_password(&config.password);

        Self {
            client,
            ebpf_table: config.ebpf_table.clone(),
            k8s_table: config.k8s_table.clone(),
        }
    }

    /// Query recent K8s resource events with optional filters.
    pub async fn get_k8s_events(
        &self,
        namespace: Option<&str>,
        resource_type: Option<&str>,
        last_minutes: u32,
    ) -> Result<Vec<K8sRow>> {
        let mut conditions = vec![format!(
            "event_time >= now() - INTERVAL {} MINUTE",
            last_minutes
        )];

        if let Some(ns) = namespace {
            conditions.push(format!("namespace = '{}'", sanitize(ns)));
        }
        if let Some(rt) = resource_type {
            conditions.push(format!("resource_type = '{}'", sanitize(rt)));
        }

        let query = format!(
            "SELECT event_time, resource_type, name, namespace, event_type, labels, resource_data \
             FROM {} WHERE {} ORDER BY event_time DESC LIMIT 100",
            self.k8s_table,
            conditions.join(" AND ")
        );

        let rows: Vec<K8sRow> = self.client.query(&query).fetch_all().await?;
        Ok(rows)
    }

    /// Query eBPF-captured HTTP traffic with optional service filters.
    pub async fn get_network_events(
        &self,
        src_service: Option<&str>,
        dst_service: Option<&str>,
        last_minutes: u32,
    ) -> Result<Vec<EbpfRow>> {
        // timestamp_ns is nanoseconds since epoch. Compute the cutoff.
        let mut conditions = vec![format!(
            "timestamp_ns >= (toUnixTimestamp(now()) - {}) * 1000000000",
            last_minutes * 60
        )];

        if let Some(src) = src_service {
            conditions.push(format!("src_service = '{}'", sanitize(src)));
        }
        if let Some(dst) = dst_service {
            conditions.push(format!("dst_service = '{}'", sanitize(dst)));
        }

        let query = format!(
            "SELECT timestamp_ns, src_service, src_namespace, dst_service, dst_namespace, \
                    src_port, dst_port, method, path, host \
             FROM {} WHERE {} ORDER BY timestamp_ns DESC LIMIT 100",
            self.ebpf_table,
            conditions.join(" AND ")
        );

        let rows: Vec<EbpfRow> = self.client.query(&query).fetch_all().await?;
        Ok(rows)
    }
}

/// Basic sanitization — strip single quotes to prevent trivial SQL injection.
/// For a production system you'd use parameterized queries, but the clickhouse
/// crate's `.bind()` doesn't support all WHERE patterns we need here.
fn sanitize(input: &str) -> String {
    input.replace('\'', "")
}
