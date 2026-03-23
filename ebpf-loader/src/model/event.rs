#[derive(Debug, Clone)]
pub struct EbpfNetworkEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub method: String,
    pub path: String,
    pub host: String,
}
