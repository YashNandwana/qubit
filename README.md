# Qubit

**eBPF-based Service Dependency Mapper for Kubernetes**

Qubit automatically discovers and maps service-to-service HTTP dependencies in Kubernetes clusters by monitoring network traffic at the kernel level. No code changes, no sidecars, no instrumentation SDKs required.

## Architecture

Qubit consists of three components that work together:

```
                         Kubernetes Cluster
 ┌──────────────────────────────────────────────────────────────────────┐
 │                                                                      │
 │   eBPF DaemonSet                      Cluster Agent                  │
 │   (every node)                        (single replica)               │
 │                                                                      │
 │   Kernel ──► ebpf-loader              Watches K8s API:               │
 │   • Captures HTTP packets             • Pods (IP ↔ service mapping)  │
 │   • Extracts method, path, host       • Services (selectors)         │
 │   • Sends events via gRPC             • ConfigMaps                   │
 │                                                                      │
 └──────────┬───────────────────────────────────────┬───────────────────┘
            │ gRPC :50051                           │ gRPC :50051
            │ SendEbpfNetworkEvent                  │ SendPodEvent
            │                                       │ SendServiceEvent
            ▼                                       ▼
     ┌─────────────────────────────────────────────────────┐
     │                     Core                             │
     │                                                      │
     │  EventIngestion (write path)                         │
     │  • EbpfAggregator ─── batch buffer ──► ClickHouse   │
     │  • K8sAggregator ──── pod cache                     │
     │  • Topology graph (in-memory)                       │
     │                                                      │
     │  QubitQuery (read path)                              │
     │  • GetTopology ──► nodes, upstream, downstream      │
     │                                                      │
     │  HTTP :9000                                          │
     │  • /ping (health check)                             │
     └─────────────────────────────────────────────────────┘
```

**Data flow:** eBPF captures raw HTTP packets on each node and sends them to Core via gRPC. The Cluster Agent watches the Kubernetes API for pod and service metadata, sending it to Core's pod cache. Core correlates the two — enriching raw IP-based events with service names and namespaces — and builds an in-memory topology graph. Clients query the topology via the `QubitQuery` gRPC service.

## How It Works

**1. Capture** — An eBPF socket filter attaches to each node's network interface and captures HTTP L7 packets. The userspace loader extracts source/destination IPs, ports, HTTP method, path, and Host header.

**2. Enrich** — The Cluster Agent watches the Kubernetes API for pod and service events. It matches pods to services via label selectors and sends the IP → service mappings to Core. Core maintains a pod cache that maps raw IPs to service names.

**3. Correlate** — When an eBPF event arrives, Core resolves the source IP via the pod cache and the destination via the Host header (e.g., `service-b.default.svc.cluster.local` → service-b in namespace default). If events arrive before the pod cache is populated, the topology self-heals when the mapping arrives later.

**4. Store** — Events are buffered and batch-written to ClickHouse. The in-memory topology graph tracks nodes (services), upstream flows (who calls this service?), and downstream flows (what does this service call?).

**5. Query** — The `QubitQuery` gRPC service exposes the full topology graph:

```bash
grpcurl -plaintext localhost:50051 qubit.QubitQuery/GetTopology
```

Returns nodes, upstream, and downstream maps keyed by `namespace/service_name`.

## Project Structure

```
qubit/
├── core/                        # Aggregation server (runs on host or as Deployment)
│   ├── src/
│   │   ├── main.rs              # Entry point — wires up servers, DB, topology
│   │   ├── aggregator/
│   │   │   ├── ebpf_aggregator  # Processes eBPF events, manages batch buffer
│   │   │   └── k8s_aggregator   # Pod/service metadata cache, topology healing
│   │   ├── server/
│   │   │   ├── grpc.rs          # EventIngestion service (write path)
│   │   │   ├── query.rs         # QubitQuery service (read path)
│   │   │   ├── http.rs          # Health endpoint
│   │   │   └── factory.rs       # Server construction
│   │   ├── topology/            # In-memory service dependency graph
│   │   ├── dao/                 # ClickHouse persistence
│   │   ├── config/              # YAML config
│   │   └── model/               # Event types, errors
│   ├── proto/qubit.proto        # gRPC service definitions
│   └── hack/                    # Makefile, ClickHouse docker-compose
│
├── cluster-agent/               # K8s metadata collector (runs in-cluster)
│   ├── src/
│   │   ├── main.rs              # Entry point — creates K8s client, starts informers
│   │   ├── kubernetes/
│   │   │   ├── informer.rs      # Generic K8s resource watcher (EventHandler trait)
│   │   │   ├── informer_factory # Creates typed informers for Pod/Service/ConfigMap
│   │   │   ├── service_registry # In-memory cache of service selectors
│   │   │   ├── pod_handler      # Maps pods to services via label matching
│   │   │   ├── service_handler  # Tracks service selector changes
│   │   │   └── configmap_handler
│   │   └── service/
│   │       └── cluster_aggregator  # gRPC client to Core
│   └── proto/qubit.proto
│
├── ebpf/                        # eBPF kernel program (socket filter)
│   └── src/main.rs
│
├── ebpf-loader/                 # Userspace eBPF loader daemon
│   └── src/
│       ├── loader/              # Loads eBPF bytecode, reads perf array
│       ├── service/             # gRPC client to Core
│       └── config/
│
├── ebpf-common/                 # Shared types between eBPF and loader
│
└── ebpf/hack/                   # K8s manifests and dev tooling
    └── k8s/
        ├── ebpf-daemonset.yaml  # eBPF loader DaemonSet
        ├── cluster-agent.yaml   # Cluster Agent Deployment + RBAC
        └── test-pods.yaml       # Test services (service-a → service-b)
```

## Prerequisites

- **Rust** (stable toolchain)
- **protoc** — `brew install protobuf` (macOS) or `apt install protobuf-compiler` (Linux)
- **ClickHouse** — for event persistence
- **Lima** — for local K8s development on macOS (`brew install lima`)
- **Kind** — Kubernetes in Docker (installed inside Lima VM)
- **grpcurl** — for testing gRPC services (`brew install grpcurl`)

## Quick Start

### 1. Start Core (macOS)

```bash
# Start ClickHouse
make -C core/hack core-up
```

This starts ClickHouse via Docker Compose and runs the Core server. Core listens on:
- `localhost:50051` — gRPC (EventIngestion + QubitQuery)
- `localhost:9000/ping` — HTTP health check

### 2. Deploy to Kind cluster (Lima VM)

```bash
# One-time setup
make -C ebpf/hack lima-create        # Create Ubuntu VM with Docker + Kind
make -C ebpf/hack vm-setup           # Install Rust toolchain in VM

# Build eBPF bytecode (runs on macOS via Docker)
make -C ebpf/hack build-ebpf

# Full deploy: Kind cluster + eBPF loader + cluster-agent + test pods
make -C ebpf/hack vm-test
```

This creates a Kind cluster inside the Lima VM with:
- **eBPF DaemonSet** — captures HTTP traffic on every node
- **Cluster Agent** — watches pods/services, sends metadata to Core
- **Test pods** — `service-a` (curl client) calls `service-b` (nginx) every 5 seconds

### 3. Query the topology

```bash
grpcurl -plaintext localhost:50051 qubit.QubitQuery/GetTopology
```

You should see `service-a` calling `service-b` and `httpbin.org`:

```json
{
  "nodes": {
    "default/service-a": { "serviceName": "service-a", "namespace": "default", "ip": "10.244.0.32" },
    "default/service-b": { "serviceName": "service-b", "namespace": "default", "ip": "10.244.0.33" }
  },
  "downstream": {
    "default/service-a": {
      "flows": [
        { "sourceService": "service-a", "destinationService": "service-b", "method": "GET", "path": "/" },
        { "sourceService": "service-a", "destinationService": "httpbin", "method": "GET", "path": "/get" }
      ]
    }
  }
}
```

### 4. Iterate

```bash
# Rebuild and redeploy after code changes
make -C ebpf/hack vm-redeploy

# View eBPF logs
make -C ebpf/hack vm-logs

# Check pod status
make -C ebpf/hack vm-status

# Chaos testing (create/update/delete K8s objects in a loop)
make -C ebpf/hack vm-chaos

# Tear down
make -C ebpf/hack vm-cleanup
```

## Configuration

### Core (`core/config.yaml`)

```yaml
app:
  http_port: 9000
  grpc_port: 50051
  ebpf_bulk_insertion_max_size: 100   # batch size before flushing to ClickHouse
  ebpf_flush_interval_secs: 5         # periodic flush interval

db:
  host: localhost
  port: 8123
  user: default
  password: "qubit"
  table:
    ebpf_network_events: ebpf_network_events
```

### Cluster Agent (`cluster-agent/config.yaml`)

```yaml
qubit_core:
  host: "192.168.5.2"     # Core address (Mac host gateway from Lima VM)
  grpc_port: 50051

kubernetes:
  namespace: ""            # empty = watch all namespaces
```

### eBPF Loader (`ebpf/hack/k8s/config.yaml`)

```yaml
qubit_core:
  host: "192.168.5.2"
  grpc_port: 50051

perf_array_name: "NETWORK_EVENTS"
ebpf_path: "/app/ebpf-bytecode"
```

## gRPC API

### Write Path — `EventIngestion`

| RPC | Description |
|-----|-------------|
| `SendEbpfNetworkEvent` | HTTP traffic event from eBPF loader |
| `SendPodEvent` | Pod created/deleted from cluster-agent |
| `SendServiceEvent` | Service created/deleted from cluster-agent |
| `SendConfigMapEvent` | ConfigMap created/deleted from cluster-agent |
| `SendServicePodMap` | Bulk pod-service mapping (initial sync) |

### Read Path — `QubitQuery`

| RPC | Description |
|-----|-------------|
| `GetTopology` | Returns full service dependency graph |

`GetTopologyResponse` contains:
- **`nodes`** — All known services, keyed by `namespace/service_name`
- **`upstream`** — For each service: who calls it (keyed by destination)
- **`downstream`** — For each service: what it calls (keyed by source)

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust |
| Async runtime | Tokio |
| eBPF framework | Aya |
| gRPC | Tonic + Prost |
| HTTP server | Axum |
| K8s client | kube-rs + k8s-openapi |
| Database | ClickHouse |
| Caching | Moka |
| Errors | thiserror + anyhow |
| Dev environment | Lima + Kind |

## Local Dev Architecture

```
 macOS Host                          Lima VM (Ubuntu)
┌─────────────────┐                ┌─────────────────────────────┐
│                 │                │  Kind Cluster (qubit-test)  │
│  Core           │◄── gRPC ──────│   ├── eBPF DaemonSet        │
│  (cargo run)    │    :50051     │   ├── Cluster Agent          │
│                 │                │   ├── service-a (curl)       │
│  ClickHouse     │                │   └── service-b (nginx)      │
│  (Docker)       │                │                               │
└─────────────────┘                └─────────────────────────────┘
   192.168.5.2                        192.168.5.15
```

Core runs natively on macOS for fast iteration. The Kind cluster inside the Lima VM hosts the eBPF DaemonSet (needs Linux kernel access), the Cluster Agent, and test workloads. All in-cluster components reach Core at `192.168.5.2:50051` (Mac host gateway).
