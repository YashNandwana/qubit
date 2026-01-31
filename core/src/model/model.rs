use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct EbpfNetworkEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub domain: String,
}

impl EbpfNetworkEvent {
    /// ClickHouse schema for this struct. Keep in sync with struct fields above.
    pub const CREATE_TABLE_SCHEMA: &'static str = "
        timestamp_ns UInt64,
        src_ip UInt32,
        dst_ip UInt32,
        src_port UInt16,
        dst_port UInt16,
        domain String
    ";
}

impl fmt::Display for EbpfNetworkEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EbpfNetworkEvent {{ ts: {}, src: {}:{}, dst: {}:{} domain: {} }}",
            self.timestamp_ns, self.src_ip, self.src_port, self.dst_ip, self.dst_port, self.domain
        )
    }
}
