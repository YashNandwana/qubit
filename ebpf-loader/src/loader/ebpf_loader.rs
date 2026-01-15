use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex};

use aya::maps::perf::AsyncPerfEventArray;
use aya::programs::SocketFilter;
use aya::util::online_cpus;
use aya::Ebpf;
use aya_log::EbpfLogger;
use bytes::BytesMut;
use ebpf_common::DnsQueryEvent;
use log::{error, info, warn};
use tokio::signal;

use crate::config::EbpfLoaderConfig;
use crate::model::EbpfNetworkEvent;
use crate::service::QubitAggregator;

const DEBUG_ASSERTION_EBPF_PATH: &str = "/workspace/ebpf/target/bpfel-unknown-none/debug/ebpf";
const RELEASE_EBPF_PATH: &str = "/workspace/ebpf/target/bpfel-unknown-none/release/ebpf";

/// Service call tracking
#[derive(Debug, Clone)]
struct ServiceCall {
    domain: String,
    count: u64,
}

type ServiceMap = Arc<Mutex<HashMap<String, Vec<ServiceCall>>>>;

pub struct EbpfLoader {
    perf_array_name: String,
    config: Arc<EbpfLoaderConfig>,
}

impl EbpfLoader {
    pub fn new(config: Arc<EbpfLoaderConfig>, perf_array_name: String) -> Self {
        Self {
            perf_array_name,
            config,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mut bpf = self.load_ebpf_program()?;
        let sock = self.create_raw_socket()?;
        self.attach_socket_filter(&mut bpf, &sock)?;

        let aggregator = Arc::new(QubitAggregator::new(self.config.clone()));

        self.spawn_event_readers(&mut bpf, aggregator)?;

        signal::ctrl_c().await?;

        Ok(())
    }

    /// Load the eBPF bytecode into the kernel
    fn load_ebpf_program(&self) -> anyhow::Result<Ebpf> {
        let ebpf_path = if cfg!(debug_assertions) {
            DEBUG_ASSERTION_EBPF_PATH
        } else {
            RELEASE_EBPF_PATH
        };

        info!("Starting eBPF Service Map Monitor");
        info!("Loading: {}", ebpf_path);

        let ebpf_data =
            std::fs::read(ebpf_path).expect("Failed to read eBPF. Run 'make build-ebpf' first.");

        let mut bpf = Ebpf::load(&ebpf_data)?;

        if let Err(e) = EbpfLogger::init(&mut bpf) {
            warn!("eBPF logger: {}", e);
        }

        Ok(bpf)
    }

    /// Create a raw packet socket for capturing network traffic
    fn create_raw_socket(&self) -> anyhow::Result<std::os::unix::io::OwnedFd> {
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
        Ok(sock)
    }

    /// Attach the socket filter program to the raw socket
    fn attach_socket_filter(
        &self,
        bpf: &mut Ebpf,
        sock: &std::os::unix::io::OwnedFd,
    ) -> anyhow::Result<()> {
        let program: &mut SocketFilter = bpf.program_mut("dns_filter").unwrap().try_into()?;
        program.load()?;
        program.attach(sock)?;
        info!("eBPF program attached!");
        Ok(())
    }

    /// Spawn async tasks to read events from each CPU's perf buffer
    fn spawn_event_readers(
        &self,
        bpf: &mut Ebpf,
        aggregator: Arc<QubitAggregator>,
    ) -> anyhow::Result<()> {
        let mut perf_array =
            AsyncPerfEventArray::try_from(bpf.take_map(&self.perf_array_name).unwrap())?;

        let cpus = online_cpus().map_err(|(msg, e)| anyhow::anyhow!("{}: {}", msg, e))?;
        info!("Spawning readers for {} CPUs", cpus.len());

        for cpu_id in cpus {
            let mut buf = perf_array.open(cpu_id, None)?;
            let agg = aggregator.clone();

            tokio::spawn(async move {
                Self::process_events(&mut buf, agg).await;
            });
        }

        Ok(())
    }

    /// Process events from a single CPU's perf buffer (static method)
    async fn process_events(
        buf: &mut aya::maps::perf::AsyncPerfEventArrayBuffer<aya::maps::MapData>,
        aggregator: Arc<QubitAggregator>,
    ) {
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
                Self::handle_dns_event(&buffers[i], &aggregator).await;
            }
        }
    }

    /// Parse and record a single DNS event (static method)
    async fn handle_dns_event(
        buf: &BytesMut,
        aggregator: &QubitAggregator,
    ) {
        if buf.len() < std::mem::size_of::<DnsQueryEvent>() {
            return;
        }

        let event = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const DnsQueryEvent) };

        let src = Self::ip_to_string(event.src_ip);
        let dst = Self::ip_to_string(event.dst_ip);

        if let Some(domain) = event.parse_query_name() {
            let ebpf_event = EbpfNetworkEvent {
                timestamp_ns: event.timestamp_ns,
                src_ip: event.src_ip,
                dst_ip: event.dst_ip,
                src_port: event.src_port,
                dst_port: event.dst_port,
                domain: domain,
            };

            if let Err(e) = aggregator.record_ebpf_event(ebpf_event).await {
                error!("Failed to send event: {}", e);
            }
        } else {
            error!("{} ──DNS──> {} [parse error]", src, dst);
        }
    }

    /// Convert a network-order u32 IP to a string
    fn ip_to_string(ip: u32) -> String {
        Ipv4Addr::from(u32::from_be(ip)).to_string()
    }
}
