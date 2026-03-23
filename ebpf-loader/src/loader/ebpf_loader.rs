use std::net::Ipv4Addr;
use std::os::unix::io::FromRawFd;
use std::sync::Arc;

use aya::maps::perf::AsyncPerfEventArray;
use aya::programs::SocketFilter;
use aya::util::online_cpus;
use aya::Ebpf;
use aya_log::EbpfLogger;
use bytes::BytesMut;
use ebpf_common::TcpPayloadEvent;
use log::{error, info, warn};
use tokio::signal;

use crate::config::EbpfLoaderConfig;
use crate::model::EbpfNetworkEvent;
use crate::service::QubitAggregator;

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
        info!("eBPF program loaded successfully");

        let sock = self.create_raw_socket()?;
        info!("Raw socket created");

        self.attach_socket_filter(&mut bpf, &sock)?;

        let aggregator = Arc::new(QubitAggregator::new(self.config.clone()));

        self.spawn_event_readers(&mut bpf, aggregator)?;

        info!("eBPF HTTP L7 Monitor running. Waiting for HTTP traffic...");
        signal::ctrl_c().await?;

        Ok(())
    }

    fn load_ebpf_program(&self) -> anyhow::Result<Ebpf> {
        let ebpf_path = &self.config.ebpf_path;

        info!("Starting eBPF HTTP L7 Monitor");
        info!("Loading: {}", ebpf_path);

        let ebpf_data =
            std::fs::read(ebpf_path).expect("Failed to read eBPF. Run 'make build-ebpf' first.");

        let mut bpf = Ebpf::load(&ebpf_data)?;

        if let Err(e) = EbpfLogger::init(&mut bpf) {
            warn!("eBPF logger: {}", e);
        }

        Ok(bpf)
    }

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

    fn attach_socket_filter(
        &self,
        bpf: &mut Ebpf,
        sock: &std::os::unix::io::OwnedFd,
    ) -> anyhow::Result<()> {
        info!("Looking for http_filter program...");
        let program: &mut SocketFilter = bpf.program_mut("http_filter").unwrap().try_into()?;
        info!("Found http_filter program, loading into kernel...");

        if let Err(e) = program.load() {
            error!("BPF verifier rejected program: {}", e);
            return Err(e.into());
        }
        info!("BPF program loaded into kernel successfully");

        if let Err(e) = program.attach(sock) {
            error!("Failed to attach to socket: {}", e);
            return Err(e.into());
        }
        info!("eBPF HTTP filter attached to socket!");
        Ok(())
    }

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

    async fn process_events(
        buf: &mut aya::maps::perf::AsyncPerfEventArrayBuffer<aya::maps::MapData>,
        aggregator: Arc<QubitAggregator>,
    ) {
        let mut buffers = (0..10)
            .map(|_| BytesMut::with_capacity(std::mem::size_of::<TcpPayloadEvent>() + 64))
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
                Self::handle_tcp_event(&buffers[i], &aggregator).await;
            }
        }
    }

    async fn handle_tcp_event(buf: &BytesMut, aggregator: &QubitAggregator) {
        if buf.len() < std::mem::size_of::<TcpPayloadEvent>() {
            return;
        }

        let event = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const TcpPayloadEvent) };

        let src = Self::ip_to_string(event.src_ip);
        let dst = Self::ip_to_string(event.dst_ip);

        // Parse HTTP in userspace
        let method = event.parse_method().unwrap_or_default();
        let path = event.parse_path().unwrap_or_default();
        let host = event.parse_host().unwrap_or_default();

        if method.is_empty() {
            return;
        }

        info!(
            "HTTP: {}:{} --> {}:{} | {} {} | host={}",
            src, event.src_port, dst, event.dst_port, method, path, host
        );

        let ebpf_event = EbpfNetworkEvent {
            timestamp_ns: event.timestamp_ns,
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port,
            dst_port: event.dst_port,
            method,
            path,
            host,
        };

        if let Err(e) = aggregator.record_ebpf_event(ebpf_event).await {
            warn!("Backend unavailable: {}", e);
        }
    }

    fn ip_to_string(ip: u32) -> String {
        Ipv4Addr::from(u32::from_be(ip)).to_string()
    }
}
