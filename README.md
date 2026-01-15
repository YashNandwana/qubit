# Qubit

**eBPF-based Service Dependency Mapper for Kubernetes**

Qubit is an observability tool that uses eBPF to automatically discover and map service-to-service dependencies in Kubernetes clusters by monitoring DNS traffic in real-time.

## ✨ Features

- **Zero-instrumentation observability** — No code changes or sidecars required
- **Real-time DNS monitoring** — Captures DNS queries at the kernel level using eBPF
- **Service dependency mapping** — Automatically discovers which services call which endpoints
- **Kubernetes-native** — Integrates with Kubernetes APIs to correlate network traffic with services
- **Low overhead** — Kernel-level packet filtering with minimal performance impact

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        Kubernetes Cluster                        │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐         ┌─────────────────────────────────┐ │
│  │   ebpf-loader   │         │            core                 │ │
│  │   (DaemonSet)   │────────▶│         (Deployment)            │ │
│  │                 │  HTTP   │                                 │ │
│  │  • DNS capture  │         │  • HTTP server                  │ │
│  │  • Event stream │         │  • Event aggregation            │ │
│  └─────────────────┘         │  • K8s controller               │ │
│         │                    └─────────────────────────────────┘ │
│         │ eBPF                                                   │
│         ▼                                                        │
│  ┌─────────────────┐                                             │
│  │  Kernel Space   │                                             │
│  │  DNS filtering  │                                             │
│  └─────────────────┘                                             │
└──────────────────────────────────────────────────────────────────┘
```

## 📁 Project Structure

```
qubit/
├── core/              # Main aggregation server
│   └── src/
│       ├── aggregator/    # eBPF event aggregation
│       ├── config/        # Configuration management
│       ├── kubernetes/    # K8s controller & informers
│       ├── model/         # Data models
│       └── server/        # HTTP API server
├── ebpf/              # eBPF program (kernel space)
│   └── src/
│       └── main.rs        # DNS socket filter
├── ebpf-common/       # Shared types between eBPF and userspace
└── ebpf-loader/       # eBPF loader daemon
    └── src/
        ├── loader/        # eBPF program loader
        ├── service/       # Aggregator client
        └── config/        # Loader configuration
```

## 🔧 Prerequisites

- **Rust** (2021 edition or newer)
- **Linux kernel** 5.4+ with eBPF support
- **bpf-linker** for compiling eBPF programs
- **Kubernetes cluster** (for full functionality)

## 🚀 Quick Start

### Build the eBPF Program

```bash
cd ebpf
cargo build --release
```

### Build and Run the Core Service

```bash
cd core
cargo build --release
cargo run
```

### Build and Run the eBPF Loader

> ⚠️ Requires root privileges and Linux with eBPF support

```bash
cd ebpf-loader
cargo build --release
sudo ./target/release/ebpf-loader
```
### Build and Run the eBPF Loader
1. Run the core service.
2. Run the sample tests with below command
```bash
cd ebpf
make build-all test-dns
```


## 🐳 Docker Deployment

The ebpf-loader runs as a privileged container to access the kernel:

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: qubit-ebpf-loader
spec:
  template:
    spec:
      containers:
      - name: ebpf-loader
        image: qubit/ebpf-loader:latest
        securityContext:
          privileged: true
        volumeMounts:
        - name: host-proc
          mountPath: /host/proc
          readOnly: true
```

## ⚙️ Configuration

### Core Service

Configure via environment variables or YAML config file:

| Variable | Description | Default |
|----------|-------------|---------|
| `QUBIT_SERVER_PORT` | HTTP server port | `8080` |
| `QUBIT_NAMESPACE` | K8s namespace to watch | `""` (all) |

### eBPF Loader

| Variable | Description | Default |
|----------|-------------|---------|
| `QUBIT_CORE_HOST` | Core service address | `localhost:8080` |

## 📊 How It Works

1. **DNS Capture**: The eBPF socket filter attaches to raw packet sockets and filters UDP traffic on port 53
2. **Event Extraction**: Source/destination IPs, ports, and queried domain names are extracted from DNS packets
3. **Userspace Processing**: Events are streamed via perf buffers to the ebpf-loader daemon
4. **Aggregation**: The core service receives events and correlates them with Kubernetes service metadata
5. **Dependency Mapping**: Service-to-service dependencies are identified based on DNS query patterns

## 🛠️ Development

### Building for Development

```bash
# Build eBPF with debug assertions
cd ebpf && cargo build

# Build loader
cd ebpf-loader && cargo build
```

### Tech Stack

- **Rust** — Systems programming language
- **Aya** — Rust eBPF library
- **Axum** — Async HTTP framework
- **Kube-rs** — Kubernetes client for Rust
- **Tokio** — Async runtime

