use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EbpfNetworkEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub domain: String,
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
