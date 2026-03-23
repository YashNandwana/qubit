#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::{map, socket_filter},
    maps::PerfEventArray,
    programs::SkBuffContext,
    EbpfContext,
};

const MAX_PAYLOAD: usize = 128;

/// TCP event with HTTP payload capture
#[repr(C)]
#[derive(Clone, Copy)]
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

#[map]
static NETWORK_EVENTS: PerfEventArray<TcpPayloadEvent> = PerfEventArray::new(0);

const ETH_P_IP: u16 = 0x0800;
const IPPROTO_TCP: u8 = 6;
const ETH_HDR_LEN: usize = 14;

#[socket_filter]
pub fn http_filter(ctx: SkBuffContext) -> i64 {
    match try_http_filter(&ctx) {
        Ok(ret) => ret,
        Err(_) => 0,
    }
}

#[inline(always)]
fn try_http_filter(ctx: &SkBuffContext) -> Result<i64, i64> {
    // Check for IPv4
    let eth_proto: u16 = u16::from_be(ctx.load(12).map_err(|_| 0i64)?);
    if eth_proto != ETH_P_IP {
        return Ok(0);
    }

    // Check for TCP
    let ip_proto: u8 = ctx.load(23).map_err(|_| 0i64)?;
    if ip_proto != IPPROTO_TCP {
        return Ok(0);
    }

    // Parse IP header length (IHL field, lower 4 bits of first byte)
    let ip_ihl: u8 = ctx.load(ETH_HDR_LEN).map_err(|_| 0i64)?;
    let ip_hdr_len = ((ip_ihl & 0x0F) as usize) * 4;

    let tcp_start = ETH_HDR_LEN + ip_hdr_len;

    // Get destination port — only capture HTTP requests, not responses
    let dst_port: u16 = u16::from_be(ctx.load(tcp_start + 2).map_err(|_| 0i64)?);
    if dst_port != 80 && dst_port != 8080 {
        return Ok(0);
    }

    let src_port: u16 = u16::from_be(ctx.load(tcp_start).map_err(|_| 0i64)?);

    // Read TCP data offset to find where payload starts
    let tcp_data_off: u8 = ctx.load(tcp_start + 12).map_err(|_| 0i64)?;
    let tcp_hdr_len = ((tcp_data_off >> 4) as usize) * 4;
    let payload_offset = tcp_start + tcp_hdr_len;

    // Check if there's actual payload (skip SYN/ACK/FIN with no data)
    let pkt_len = ctx.len() as usize;
    if payload_offset >= pkt_len {
        return Ok(0);
    }

    // Compute available payload and clamp to MAX_PAYLOAD.
    let avail = pkt_len - payload_offset;
    let copy_len = if avail < MAX_PAYLOAD {
        avail
    } else {
        MAX_PAYLOAD
    };

    // IMPORTANT: read_volatile prevents LLVM from proving copy_len >= 1
    // (which it can from the `payload_offset >= pkt_len` guard above) and
    // removing the zero-check below. Without this barrier, the compiler
    // emits JGT instead of JGE, so the verifier sees copy_len ∈ [0,127]
    // and rejects the bpf_skb_load_bytes call as "invalid zero-sized read".
    let copy_len = unsafe { core::ptr::read_volatile(&copy_len) };
    if copy_len == 0 || copy_len > MAX_PAYLOAD {
        return Ok(0);
    }

    // Check first byte — HTTP methods start with an ASCII letter
    let first_byte: u8 = ctx.load(payload_offset).map_err(|_| 0i64)?;
    if !first_byte.is_ascii_uppercase() {
        return Ok(0);
    }

    // Extract IP addresses
    let src_ip: u32 = ctx.load(26).map_err(|_| 0i64)?;
    let dst_ip: u32 = ctx.load(30).map_err(|_| 0i64)?;

    let mut event = TcpPayloadEvent {
        timestamp_ns: unsafe { bpf_ktime_get_ns() },
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        payload_len: 0,
        _padding: 0,
        payload: [0u8; MAX_PAYLOAD],
    };

    // Copy payload via bpf_skb_load_bytes with a length the verifier can prove > 0
    let ret = unsafe {
        aya_ebpf::helpers::gen::bpf_skb_load_bytes(
            ctx.as_ptr() as *const _,
            payload_offset as u32,
            event.payload.as_mut_ptr() as *mut _,
            copy_len as u32,
        )
    };
    if ret < 0 {
        return Ok(0);
    }
    event.payload_len = copy_len as u16;

    NETWORK_EVENTS.output(ctx, &event, 0);

    Ok(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
