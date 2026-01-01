#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::{map, socket_filter},
    maps::PerfEventArray,
    programs::SkBuffContext,
};
use aya_log_ebpf::info;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DnsQueryEvent {
    pub timestamp_ns: u64,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub query_start: [u8; 16],
}

#[map]
static NETWORK_EVENTS: PerfEventArray<DnsQueryEvent> = PerfEventArray::new(0);

const ETH_P_IP: u16 = 0x0800;
const IPPROTO_UDP: u8 = 17;
const DNS_PORT: u16 = 53;

#[socket_filter]
pub fn dns_filter(ctx: SkBuffContext) -> i64 {
    match try_dns_filter(&ctx) {
        Ok(ret) => ret,
        Err(_) => 0,
    }
}

#[inline(always)]
fn try_dns_filter(ctx: &SkBuffContext) -> Result<i64, i64> {
    let eth_proto: u16 = u16::from_be(ctx.load(12).map_err(|_| 0i64)?);
    if eth_proto != ETH_P_IP {
        return Ok(0);
    }

    let ip_proto: u8 = ctx.load(23).map_err(|_| 0i64)?;
    if ip_proto != IPPROTO_UDP {
        return Ok(0);
    }

    let src_ip: u32 = ctx.load(26).map_err(|_| 0i64)?;
    let dst_ip: u32 = ctx.load(30).map_err(|_| 0i64)?;
    let src_port: u16 = u16::from_be(ctx.load(34).map_err(|_| 0i64)?);
    let dst_port: u16 = u16::from_be(ctx.load(36).map_err(|_| 0i64)?);

    if dst_port != DNS_PORT {
        return Ok(0);
    }

    let query_start = get_query_start(ctx)?;

    let event = DnsQueryEvent {
        timestamp_ns: unsafe { bpf_ktime_get_ns() },
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        query_start,
    };

    info!(ctx, "DNS from {}", u32::from_be(src_ip));
    NETWORK_EVENTS.output(ctx, &event, 0);

    Ok(-1)
}

fn get_query_start(ctx: &SkBuffContext) -> Result<[u8; 16], i64> {
    // Read DNS query name (first 16 bytes after DNS header)
    // DNS starts at offset 42 (14 eth + 20 IP + 8 UDP)
    // DNS header is 12 bytes, so query name starts at offset 54
    let q0: u32 = ctx.load(54)?; // Bytes 0-3 of query
    let q1: u32 = ctx.load(58)?; // Bytes 4-7
    let q2: u32 = ctx.load(62)?; // Bytes 8-11
    let q3: u32 = ctx.load(66)?; // Bytes 12-15

    let mut query_start = [0u8; 16];
    query_start[0..4].copy_from_slice(&q0.to_ne_bytes());
    query_start[4..8].copy_from_slice(&q1.to_ne_bytes());
    query_start[8..12].copy_from_slice(&q2.to_ne_bytes());
    query_start[12..16].copy_from_slice(&q3.to_ne_bytes());

    Ok(query_start)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
