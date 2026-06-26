//! Common types shared between eBPF and userspace

#![cfg_attr(not(feature = "user"), no_std)]

const MAX_PAYLOAD: usize = 128;

/// Raw TCP payload event - kernel captures, userspace parses HTTP
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TcpPayloadEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub payload_len: u16,
    pub _padding: u16,
    pub payload: [u8; MAX_PAYLOAD],
}

impl Default for TcpPayloadEvent {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            src_ip: 0,
            dst_ip: 0,
            src_port: 0,
            dst_port: 0,
            payload_len: 0,
            _padding: 0,
            payload: [0u8; MAX_PAYLOAD],
        }
    }
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for TcpPayloadEvent {}

#[cfg(feature = "user")]
impl TcpPayloadEvent {
    fn payload_bytes(&self) -> &[u8] {
        let len = (self.payload_len as usize).min(self.payload.len());
        &self.payload[..len]
    }

    pub fn parse_method(&self) -> Option<String> {
        let payload = self.payload_bytes();
        // Find first space in raw bytes — only the method needs to be valid UTF-8
        let space_pos = payload.iter().position(|&b| b == b' ')?;
        let method = core::str::from_utf8(&payload[..space_pos]).ok()?;
        Some(method.to_string())
    }

    pub fn parse_path(&self) -> Option<String> {
        let payload = self.payload_bytes();
        // "GET /path HTTP/1.1" — path is between first and second space
        let first_space = payload.iter().position(|&b| b == b' ')?;
        let rest = &payload[first_space + 1..];
        let second_space = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
        let path = core::str::from_utf8(&rest[..second_space]).ok()?;
        Some(path.to_string())
    }

    pub fn parse_host(&self) -> Option<String> {
        let payload = self.payload_bytes();
        // Search for "Host:" (case-insensitive) in raw bytes
        let host_pos = payload
            .windows(5)
            .position(|w| w.eq_ignore_ascii_case(b"Host:"))?;

        let after_host = &payload[host_pos + 5..];
        // Skip whitespace
        let start = after_host.iter().position(|&b| b != b' ' && b != b'\t')?;
        let value = &after_host[start..];
        // Find end of header line
        let end = value
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .unwrap_or(value.len());
        let host = core::str::from_utf8(&value[..end]).ok()?;
        // Strip port if present (e.g. "service-b:80" -> "service-b")
        let host = host.split(':').next().unwrap_or(host);
        Some(host.to_string())
    }

    pub fn payload_str(&self) -> String {
        let payload = self.payload_bytes();
        String::from_utf8_lossy(payload)
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    }
}

// Tests are only compiled when the "user" feature is enabled, because the
// parsing methods are feature-gated. Run with:
//   cargo test -p ebpf-common --features user
#[cfg(all(test, feature = "user"))]
mod tests {
    use super::TcpPayloadEvent;

    // Helper: build a TcpPayloadEvent from a raw byte slice.
    // Bytes beyond MAX_PAYLOAD are silently truncated.
    fn make_event(payload: &[u8]) -> TcpPayloadEvent {
        let mut event = TcpPayloadEvent::default();
        let copy_len = payload.len().min(event.payload.len());
        event.payload[..copy_len].copy_from_slice(&payload[..copy_len]);
        event.payload_len = copy_len as u16;
        event
    }

    #[test]
    fn parse_method_get() {
        let event = make_event(b"GET /index HTTP/1.1\r\nHost: svc\r\n\r\n");
        assert_eq!(event.parse_method(), Some("GET".to_string()));
    }

    #[test]
    fn parse_method_post() {
        let event = make_event(b"POST /api/v1 HTTP/1.1\r\nHost: svc\r\n\r\n");
        assert_eq!(event.parse_method(), Some("POST".to_string()));
    }

    #[test]
    fn parse_method_malformed_no_space() {
        // No space in payload → cannot extract method
        let event = make_event(b"NOTHTTP");
        assert_eq!(event.parse_method(), None);
    }

    #[test]
    fn parse_path_standard() {
        let event = make_event(b"GET /api/v1/users HTTP/1.1\r\n");
        assert_eq!(event.parse_path(), Some("/api/v1/users".to_string()));
    }

    #[test]
    fn parse_path_root() {
        let event = make_event(b"GET / HTTP/1.1\r\n");
        assert_eq!(event.parse_path(), Some("/".to_string()));
    }

    #[test]
    fn parse_host_present() {
        let event =
            make_event(b"GET / HTTP/1.1\r\nHost: service-b.default\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(event.parse_host(), Some("service-b.default".to_string()));
    }

    #[test]
    fn parse_host_port_stripped() {
        // "Host: service-b:80" — port suffix should be stripped
        let event = make_event(b"GET / HTTP/1.1\r\nHost: service-b:80\r\n\r\n");
        assert_eq!(event.parse_host(), Some("service-b".to_string()));
    }

    #[test]
    fn parse_host_absent() {
        // No Host header present (e.g. raw TCP, health probes)
        let event = make_event(b"GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(event.parse_host(), None);
    }
}
