# Qubit

**eBPF-based Service Dependency Mapper for Kubernetes**

Qubit is an observability tool that uses eBPF to automatically discover and map service-to-service HTTP dependencies in Kubernetes clusters by monitoring network traffic at the kernel level.

## Features

- **Zero-instrumentation observability** — No code changes or sidecars required
- **Real-time HTTP monitoring** — Captures L7 traffic at the kernel level using eBPF
- **Service dependency mapping** — Automatically discovers which services call which endpoints
- **Kubernetes-native** — Integrates with Kubernetes APIs to correlate traffic with services
- **gRPC event transport** — High-performance event ingestion via gRPC (tonic/protobuf)
- **Low overhead** — Kernel-level packet filtering with minimal performance impact

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        Kubernetes Cluster                        │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐         ┌─────────────────────────────────┐ │
│  │   ebpf-loader   │         │             core                │ │
│  │   (DaemonSet)   │────────▶│          (Deployment)           │ │
│  │                 │  gRPC   │                                 │ │
│  │  • HTTP capture │ :50051  │  • gRPC server (event ingestion)│ │
│  │  • Event stream │         │  • HTTP server (health :9000)   │ │
│  └─────────────────┘         │  • Event aggregation            │ │
│         │                    │  • K8s controller               │ │
│         │ eBPF               │  • Topology graph               │ │
│         ▼                    └──────────────┬──────────────────┘ │
│  ┌─────────────────┐                        │                    │
│  │  Kernel Space   │                        ▼                    │
│  │  HTTP filtering │               ┌─────────────────┐           │
│  └─────────────────┘               │   ClickHouse    │           │
│                                    └─────────────────┘           │
└──────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
qubit/
├── core/                  # Main aggregation server
│   ├── proto/             # Protobuf definitions (qubit.proto)
│   └── src/
│       ├── aggregator/    # eBPF event aggregation + topology
│       ├── config/        # Configuration management
│       ├── dao/           # ClickHouse persistence
│       ├── kubernetes/    # K8s controller & informers
│       ├── model/         # Data models
│       ├── server/        # HTTP + gRPC servers, server factory
│       ├── service/       # K8s service layer
│       └── topology/      # Service dependency graph
├── ebpf/                  # eBPF program (kernel space)
│   └── src/main.rs        # HTTP socket filter
├── ebpf-common/           # Shared types between eBPF and userspace
└── ebpf-loader/           # eBPF loader daemon
    ├── proto/             # Protobuf definitions (qubit.proto)
    └── src/
        ├── loader/        # eBPF program loader
        ├── model/         # Event types
        ├── proto/         # Generated gRPC client code
        ├── service/       # gRPC client (QubitAggregator)
        └── config/        # Loader configuration
```

## Prerequisites

- **Rust** (2021 edition or newer)
- **protoc** (protobuf compiler) — `brew install protobuf` on macOS, `apt-get install protobuf-compiler` on Linux
- **Linux kernel** 5.4+ with eBPF support (for ebpf-loader)
- **ClickHouse** (for core persistence)
- **Kubernetes cluster** (for full functionality)

## Quick Start

### Run the Core Service (macOS or Linux)

```bash
cd core
cargo run
```

Core exposes:
- `localhost:9000/ping` — health check
- `localhost:50051` — gRPC event ingestion (`qubit.EventIngestion/SendEbpfNetworkEvent`)

### Test gRPC Ingestion

```bash
# using grpcurl
grpcurl -plaintext -d '{
  "src_ip": 16777343,
  "dst_ip": 16777343,
  "src_port": 8080,
  "dst_port": 1234,
  "method": "GET",
  "path": "/api/v1/users",
  "host": "service-b.default.svc.cluster.local"
}' localhost:50051 qubit.EventIngestion/SendEbpfNetworkEvent

# or using the built-in example client
cargo run --example send_ebpf_event
```

### Deploy ebpf-loader to Kind (Lima VM)

```bash
# one-time setup
brew install lima
make -C ebpf/hack lima-create     # creates Ubuntu VM with Docker + Kind
make -C ebpf/hack vm-setup        # installs Rust in VM

# build eBPF bytecode (macOS, via Docker)
make -C ebpf/hack build-ebpf

# first-time deploy
make -C ebpf/hack vm-test

# iterating on loader changes
make -C ebpf/hack vm-redeploy

# view events
make -C ebpf/hack vm-logs
```

## Configuration

### Core (`core/config.yaml`)

```yaml
app:
  http_port: 9000      # health endpoint
  grpc_port: 50051     # event ingestion
  upstream: http://localhost:8080
db:
  host: localhost
  port: 8123
  user: default
  password: "qubit"
  table:
    ebpf_network_events: ebpf_network_events
kubernetes:
  in_cluster: true
  namespace: ""
```

### eBPF Loader (`ebpf-loader/config.yaml` / `ebpf/hack/k8s/config.yaml`)

```yaml
qubit_core:
  host: "host.lima.internal"
  grpc_port: 50051
perf_array_name: "NETWORK_EVENTS"
ebpf_path: "/app/ebpf-bytecode"
```

## How It Works

1. **HTTP Capture**: The eBPF socket filter attaches to raw packet sockets and captures HTTP L7 traffic
2. **Event Extraction**: Source/destination IPs, ports, HTTP method, path, and host header are parsed in userspace
3. **gRPC Transport**: Events are streamed via gRPC (`SendEbpfNetworkEvent`) from the ebpf-loader to core
4. **Aggregation**: Core persists events to ClickHouse and updates an in-memory topology graph
5. **Dependency Mapping**: Service-to-service dependencies are identified from the HTTP host headers

## Tech Stack

- **Rust** — Systems programming language
- **Aya** — Rust eBPF library
- **Tonic / Prost** — gRPC framework and protobuf runtime
- **Axum** — Async HTTP framework (health endpoint)
- **Kube-rs** — Kubernetes client for Rust
- **ClickHouse** — Event storage
- **Tokio** — Async runtime
- **Lima + Kind** — Local Kubernetes development environment
