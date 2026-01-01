//! Common types shared between eBPF and userspace

#![cfg_attr(not(feature = "user"), no_std)]

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DnsQueryEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub query_start: [u8; 16],
}

impl Default for DnsQueryEvent {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            src_ip: 0,
            dst_ip: 0,
            src_port: 0,
            dst_port: 0,
            query_start: [0u8; 16],
        }
    }
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for DnsQueryEvent {}

#[cfg(feature = "user")]
impl DnsQueryEvent {
    /// Parse DNS query name from the first 16 bytes
    pub fn parse_query_name(&self) -> Option<String> {
        let mut result = String::new();
        let mut offset = 0usize;

        loop {
            if offset >= 16 {
                break;
            }

            let label_len = self.query_start[offset] as usize;

            if label_len == 0 {
                break;
            }

            if label_len > 63 || offset + 1 + label_len > 16 {
                break;
            }

            offset += 1;

            if !result.is_empty() {
                result.push('.');
            }

            for i in 0..label_len {
                if offset + i >= 16 {
                    break;
                }
                let c = self.query_start[offset + i];
                if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' {
                    result.push(c as char);
                }
            }

            offset += label_len;
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}
