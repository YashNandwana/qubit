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
    /// Convert to DB-ready event with human-readable IP strings
    pub fn into_event(self) -> EbpfNetworkEvent {
        let src_ip = Ipv4Addr::from(u32::from_be(self.src_ip)).to_string();
        let dst_ip = Ipv4Addr::from(u32::from_be(self.dst_ip)).to_string();
        EbpfNetworkEvent {
            timestamp_ns: self.timestamp_ns,
            src_ip,
            dst_ip,
            src_port: self.src_port,
            dst_port: self.dst_port,
            method: self.method,
            path: self.path,
            host: self.host,
        }
    }
}

/// Event stored in ClickHouse (IPs as readable strings)
#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct EbpfNetworkEvent {
    pub timestamp_ns: u64,
    pub src_ip: String,
    pub dst_ip: String,
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
        src_ip String,
        dst_ip String,
        src_port UInt16,
        dst_port UInt16,
        method String,
        path String,
        host String
    ";
}

impl fmt::Display for EbpfNetworkEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP: {}:{} --> {}:{} | {} {} | host={}",
            self.src_ip,
            self.src_port,
            self.dst_ip,
            self.dst_port,
            self.method,
            self.path,
            self.host
        )
    }
}
