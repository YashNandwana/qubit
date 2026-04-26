use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::Ipv4Addr;

/// Incoming event from the eBPF loader (IPs as u32 from network packets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EbpfNetworkEventInput {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub method: String,
    pub path: String,
    pub host: String,
}

impl EbpfNetworkEventInput {
    /// Convert raw u32 IPs from the eBPF packet to human-readable strings.
    pub fn src_ip_str(&self) -> String {
        Ipv4Addr::from(u32::from_be(self.src_ip)).to_string()
    }

    pub fn dst_ip_str(&self) -> String {
        Ipv4Addr::from(u32::from_be(self.dst_ip)).to_string()
    }
}

/// Event stored in ClickHouse — uses resolved service names, not raw IPs.
/// The AI agent queries by service name ("show me traffic from service-a"),
/// so storing resolved names avoids a lookup at query time.
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct EbpfNetworkEvent {
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

impl EbpfNetworkEvent {
    /// ClickHouse schema for this struct. Keep in sync with struct fields above.
    pub const CREATE_TABLE_SCHEMA: &'static str = "
        timestamp_ns UInt64,
        src_service String,
        src_namespace String,
        dst_service String,
        dst_namespace String,
        src_port UInt16,
        dst_port UInt16,
        method String,
        path String,
        host String
    ";
}

/// K8s resource event stored in ClickHouse.
///
/// `event_time` uses ClickHouse's `DateTime` type with a server-side DEFAULT
/// of `now()`. On the Rust side we set it explicitly so inserts don't rely on
/// server time. The table has a 1-day TTL on this column — ClickHouse
/// automatically drops rows older than 24 hours during background merges.
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct K8sResourceEvent {
    pub event_time: u32,       // DateTime in ClickHouse = epoch seconds as u32
    pub resource_type: String, // "Deployment", "Event", "Node", etc.
    pub name: String,
    pub namespace: String,
    pub event_type: String,    // "Applied" or "Deleted"
    pub labels: String,        // JSON-encoded label map
    pub resource_data: String, // Resource-specific JSON (Event reason/message, etc.)
}

impl K8sResourceEvent {
    pub const CREATE_TABLE_SCHEMA: &'static str = "
        event_time DateTime DEFAULT now(),
        resource_type String,
        name String,
        namespace String,
        event_type String,
        labels String,
        resource_data String
    ";
}

impl fmt::Display for EbpfNetworkEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP: {}/{}:{} --> {}/{}:{} | {} {} | host={}",
            self.src_namespace,
            self.src_service,
            self.src_port,
            self.dst_namespace,
            self.dst_service,
            self.dst_port,
            self.method,
            self.path,
            self.host
        )
    }
}
