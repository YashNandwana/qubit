use aya::{maps::perf::AsyncPerfEventArray, programs::SocketFilter, util::online_cpus, Ebpf};
use aya_log::EbpfLogger;
use bytes::BytesMut;
use ebpf_common::DnsQueryEvent;
use log::{error, info, warn};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex};
use tokio::signal;

const EBPF_NETWORK_MAP: &str = "NETWORK_EVENTS";

/// Service call tracking
#[derive(Debug, Clone)]
struct ServiceCall {
    domain: String,
    count: u64,
}

type ServiceMap = Arc<Mutex<HashMap<String, Vec<ServiceCall>>>>;

fn ip_to_string(ip: u32) -> String {
    Ipv4Addr::from(u32::from_be(ip)).to_string()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let ebpf_path = if cfg!(debug_assertions) {
        "/workspace/ebpf/target/bpfel-unknown-none/debug/ebpf"
    } else {
        "/workspace/ebpf/target/bpfel-unknown-none/release/ebpf"
    };

    info!("Starting eBPF Service Map Monitor");
    info!("Loading: {}", ebpf_path);

    let ebpf_data =
        std::fs::read(ebpf_path).expect("Failed to read eBPF. Run 'make build-ebpf' first.");

    let mut bpf = Ebpf::load(&ebpf_data)?;

    if let Err(e) = EbpfLogger::init(&mut bpf) {
        warn!("eBPF logger: {}", e);
    }

    // Create raw socket
    let sock = unsafe {
        let fd = libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            (libc::ETH_P_ALL as i16).to_be() as i32,
        );
        if fd < 0 {
            return Err(anyhow::anyhow!("Failed to create raw socket"));
        }
        std::os::unix::io::OwnedFd::from_raw_fd(fd)
    };

    let program: &mut SocketFilter = bpf.program_mut("dns_filter").unwrap().try_into()?;
    program.load()?;
    program.attach(&sock)?;

    info!("EBPF Program Attached!");

    let service_map: ServiceMap = Arc::new(Mutex::new(HashMap::new()));
    let service_map_clone = service_map.clone();

    let mut perf_array = AsyncPerfEventArray::
                            try_from(bpf.take_map(EBPF_NETWORK_MAP).unwrap())?;

    let cpus = online_cpus().map_err(|(msg, e)| anyhow::anyhow!("{}: {}", msg, e))?;
    for cpu_id in cpus {
        let mut buf = perf_array.open(cpu_id, None)?;
        let map = service_map_clone.clone();

        tokio::spawn(async move {
            let mut buffers = (0..10)
                .map(|_| BytesMut::with_capacity(std::mem::size_of::<DnsQueryEvent>() + 64))
                .collect::<Vec<_>>();

            loop {
                let events = match buf.read_events(&mut buffers).await {
                    Ok(e) => e,
                    Err(e) => {
                        error!("Read error: {}", e);
                        continue;
                    }
                };

                for i in 0..events.read {
                    let buf = &buffers[i];
                    if buf.len() >= std::mem::size_of::<DnsQueryEvent>() {
                        let event = unsafe {
                            std::ptr::read_unaligned(buf.as_ptr() as *const DnsQueryEvent)
                        };

                        let src = ip_to_string(event.src_ip);
                        let dst = ip_to_string(event.dst_ip);

                        // Parse DNS query name from payload
                        if let Some(domain) = event.parse_query_name() {
                            println!("{} ──DNS──> {} [{}]", src, dst, domain);

                            // Update service map
                            let mut map = map.lock().unwrap();
                            let calls = map.entry(src).or_insert_with(Vec::new);

                            if let Some(call) = calls.iter_mut().find(|c| c.domain == domain) {
                                call.count += 1;
                            } else {
                                calls.push(ServiceCall { domain, count: 1 });
                            }
                        } else {
                            println!("{} ──DNS──> {} [parse error]", src, dst);
                        }
                    }
                }
            }
        });
    }

    signal::ctrl_c().await?;

    println!();
    println!("----------SUMMARY---------");

    let map = service_map.lock().unwrap();
    if map.is_empty() {
        println!("No DNS queries captured.");
    } else {
        for (source, calls) in map.iter() {
            println!("Source: {}", source);
            for call in calls {
                println!("── → {} (queries: {})", call.domain, call.count);
            }
            println!();
        }
    }

    Ok(())
}
