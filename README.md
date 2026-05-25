# Qubit

**eBPF-based Service Dependency Mapper for Kubernetes**

Qubit automatically discovers and maps service-to-service HTTP dependencies in Kubernetes clusters by monitoring network traffic at the kernel level. No code changes, no sidecars, no instrumentation SDKs required.

## Architecture

```
                         Kubernetes Cluster
 ┌──────────────────────────────────────────────────────────────────────┐
 │                                                                      │
 │   eBPF DaemonSet                      Cluster Agent                  │
 │   (every node)                        (single replica)               │
 │                                                                      │
 │   Kernel ──► ebpf-loader              Watches K8s API:               │
 │   • Captures HTTP packets             • Pods (IP ↔ service mapping)  │
 │   • Extracts method, path, host       • Services, ConfigMaps         │
 │   • Sends events via gRPC             • Deployments, Ingresses, etc. │
 │                                       • Parses envoy.yaml ConfigMap  │
 │                                         → domain → service routes    │
 └──────────┬───────────────────────────────────────┬───────────────────┘
            │ gRPC :50051                           │ gRPC :50051
            │ SendEbpfNetworkEvent                  │ SendPodEvent
            │                                       │ SendServicePodMap
            │                                       │ SendEnvoyRoutes
            │                                       │ SendK8sResourceEvent
            ▼                                       ▼
     ┌─────────────────────────────────────────────────────┐
     │                     Core                             │
     │                                                      │
     │  EventIngestion (write path)                         │
     │  • EbpfAggregator ─── batch buffer ──► ClickHouse   │
     │    └── destination resolution (3-tier):              │
     │        1. EnvoyDomainCache (Host → service)          │
     │        2. Pod cache (dst IP → service)               │
     │        3. parse_k8s_host (DNS name heuristic)        │
     │  • K8sAggregator ──── pod cache + topology          │
     │                                                      │
     │  QubitQuery (read path)                              │
     │  • GetTopology ──► nodes, upstream, downstream      │
     │                                                      │
     │  HTTP :9000  /ping                                   │
     └──────────────────────────┬──────────────────────────┘
                                │ gRPC + ClickHouse queries
                                ▼
     ┌─────────────────────────────────────────────────────┐
     │                  MCP Server                          │
     │                                                      │
     │  stdio JSON-RPC (Model Context Protocol)            │
     │  • get_topology          full service graph          │
     │  • get_service_dependencies  per-service view        │
     │  • get_k8s_events        recent K8s resource events │
     │  • get_network_events    raw eBPF HTTP traffic       │
     └─────────────────────────────────────────────────────┘
                                ▲
                                │ Claude Code / AI client
```

**Data flow:**

1. **Capture** — An eBPF socket filter attaches to each node's network interface and captures outbound HTTP packets. The userspace loader extracts source/destination IPs, ports, HTTP method, path, and Host header, then forwards events to Core via gRPC.

2. **Enrich (pod metadata)** — The Cluster Agent watches the Kubernetes API for pod and service events. It matches pods to services via label selectors and sends IP → service mappings to Core. On startup and every 30 seconds it re-sends the full map so Core recovers correctly after a restart.

3. **Enrich (Envoy routes)** — When the Cluster Agent sees a ConfigMap containing an `envoy.yaml` key, it parses the Envoy static config — extracting cluster endpoint FQDNs and virtual host domains — and pushes the domain → service mappings to Core via `SendEnvoyRoutes`. This means services routed through an Envoy proxy (where the Host header is a virtual hostname, not a K8s FQDN) resolve correctly with no manual setup.

4. **Correlate** — When an eBPF event arrives, Core resolves the source IP via the pod cache (drops the event if the IP is not yet known, preventing raw IPs from leaking into the topology). The destination is resolved in priority order: Envoy cache → pod cache → K8s DNS name heuristic.

5. **Store** — Events are buffered and batch-written to ClickHouse. The in-memory topology graph tracks nodes (services), upstream flows (who calls this service?), and downstream flows (what does this service call?). Each service pair is deduplicated — only the first event per edge is persisted and added to the graph.

6. **Query** — The `QubitQuery` gRPC service exposes the full topology graph.

## Project Structure

```
qubit/
├── core/                        # Aggregation server (runs on host or as Deployment)
│   ├── src/
│   │   ├── main.rs              # Entry point — wires up servers, DB, topology
│   │   ├── lib.rs               # Library root (used by load-tests)
│   │   ├── aggregator/
│   │   │   ├── ebpf_aggregator  # Processes eBPF events, destination resolution
│   │   │   └── k8s_aggregator   # Pod/service cache, topology healing
│   │   ├── server/
│   │   │   ├── grpc.rs          # EventIngestion + QubitQuery gRPC handlers
│   │   │   ├── http.rs          # Health endpoint (/ping)
│   │   │   ├── factory.rs       # Server construction
│   │   │   └── query.rs         # Read-path query handler
│   │   ├── envoy/               # EnvoyDomainCache (populated by cluster-agent)
│   │   ├── topology/            # In-memory service dependency graph
│   │   ├── dao/                 # ClickHouse persistence
│   │   ├── config/              # YAML config
│   │   └── model/               # Event types, errors
│   ├── proto/qubit.proto        # gRPC service definitions
│   └── hack/                    # Makefile, ClickHouse docker-compose
│
├── cluster-agent/               # K8s metadata collector (runs in-cluster)
│   ├── src/
│   │   ├── kubernetes/
│   │   │   ├── informer.rs      # Generic K8s resource watcher (EventHandler trait)
│   │   │   ├── informer_factory # Typed informers (Pod, Service, ConfigMap,
│   │   │   │                    #   Deployment, ReplicaSet, Ingress, HPA, Node,
│   │   │   │                    #   Rollout, ExternalSecret, HTTPProxy, VirtualService)
│   │   │   ├── configmap_handler# Detects envoy.yaml → triggers route push
│   │   │   ├── envoy_parser     # Parses Envoy static YAML → domain→service mappings
│   │   │   ├── service_registry # In-memory cache of service selectors
│   │   │   ├── pod_handler      # Maps pods to services via label matching
│   │   │   └── service_handler  # Tracks service selector changes
│   │   └── service/
│   │       └── cluster_aggregator  # gRPC client to Core
│   └── proto/qubit.proto
│
├── ebpf/                        # eBPF kernel program (TC socket filter)
│   └── src/main.rs
│
├── ebpf-loader/                 # Userspace eBPF loader daemon
│   └── src/
│       ├── loader/              # Loads eBPF bytecode, reads perf array
│       ├── service/             # gRPC client to Core
│       └── config/
│
├── ebpf-common/                 # Shared types between eBPF kernel and loader
│
├── load-tests/                  # Traffic generator
│   └── src/main.rs              # Two streams: K8s events + HTTP traffic
│                                # Run via: make -C ebpf/hack vm-load-gen
│
├── mcp-server/                  # MCP server (AI assistant interface)
│   └── src/
│       ├── main.rs              # stdio JSON-RPC transport
│       ├── tools.rs             # MCP tool definitions
│       ├── grpc_client.rs       # Wraps Core's QubitQuery gRPC service
│       ├── ch_client.rs         # Queries ClickHouse for raw events
│       └── config.rs
│
└── ebpf/hack/                   # K8s manifests and dev tooling
    └── k8s/
        ├── ebpf-daemonset.yaml  # eBPF loader DaemonSet
        ├── cluster-agent.yaml   # Cluster Agent Deployment + RBAC
        ├── envoy-proxy.yaml     # Envoy proxy + ConfigMap (static config)
        └── test-pods.yaml       # Test services (service-a → service-b via Envoy)
```

## Prerequisites

- **Rust** (stable toolchain)
- **protoc** — `brew install protobuf` (macOS) or `apt install protobuf-compiler` (Linux)
- **ClickHouse** — for event persistence
- **Lima** — for local K8s development on macOS (`brew install lima`)
- **Kind** — Kubernetes in Docker (installed inside Lima VM)
- **Docker** — for ClickHouse and eBPF bytecode compilation
- **grpcurl** — for testing gRPC services (`brew install grpcurl`)

## Quick Start

### 1. Start Core (macOS)

```bash
make -C core/hack core-up
```

Starts ClickHouse via Docker Compose and runs Core. Listens on:
- `localhost:50051` — gRPC (EventIngestion + QubitQuery)
- `localhost:9000` — HTTP health check

### 2. Deploy to Kind cluster (Lima VM)

```bash
# One-time setup
make -C ebpf/hack lima-create        # Create Ubuntu VM with Docker + Kind
make -C ebpf/hack vm-setup           # Install Rust toolchain in VM
make -C ebpf/hack build-ebpf         # Compile eBPF bytecode (via Docker, macOS)
make -C ebpf/hack vm-cluster-create  # Create Kind cluster
make -C ebpf/hack vm-envoy-pull      # Pre-load Envoy image into Kind

# Full first-time deploy
make -C ebpf/hack vm-test
```

Deploys into Kind:
- **eBPF DaemonSet** — captures HTTP traffic on every node
- **Cluster Agent** — watches pods/services/ConfigMaps, sends metadata and Envoy routes to Core
- **Envoy proxy** — routes traffic between test services
- **Test pods** — `service-a` (curl) calls `service-b` (nginx) through Envoy every 5 seconds

### 3. Iterate

```bash
# Rebuild and redeploy after code changes
make -C ebpf/hack vm-redeploy

# Check pod status
make -C ebpf/hack vm-status

# Follow eBPF loader logs
make -C ebpf/hack vm-logs

# Generate load (K8s events + HTTP traffic)
make -C ebpf/hack vm-load-gen
make -C ebpf/hack vm-load-gen K8S_RPS=50 HTTP_RPS=100 DURATION=120
```

### 4. Query the topology

```bash
grpcurl -plaintext localhost:50051 qubit.QubitQuery/GetTopology
```

Expected: `service-a → service-b` (not `service-a → envoy-proxy`). The Envoy route resolution maps the `envoy-proxy.default.svc` Host header back to `service-b`.

## Local Dev Architecture

```
 macOS Host                          Lima VM (Ubuntu)
┌─────────────────┐                ┌─────────────────────────────┐
│                 │                │  Kind Cluster (qubit-test)  │
│  Core           │◄── gRPC ──────│   ├── eBPF DaemonSet        │
│  (cargo run)    │    :50051     │   ├── Cluster Agent          │
│                 │                │   ├── Envoy proxy            │
│  ClickHouse     │                │   ├── service-a (curl)       │
│  (Docker)       │                │   └── service-b (nginx)      │
└─────────────────┘                └─────────────────────────────┘
   192.168.5.2                        192.168.5.15
```

Core runs natively on macOS for fast iteration. The Kind cluster inside the Lima VM hosts the eBPF DaemonSet (needs Linux kernel access), the Cluster Agent, Envoy, and test workloads. All in-cluster components reach Core at `192.168.5.2:50051` (Mac host gateway).

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
    k8s_resource_events: k8s_resource_events
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

| RPC | Sender | Description |
|-----|--------|-------------|
| `SendEbpfNetworkEvent` | eBPF loader | HTTP packet event (src/dst IP, method, path, host) |
| `SendPodEvent` | cluster-agent | Pod created/deleted |
| `SendServiceEvent` | cluster-agent | Service created/deleted |
| `SendConfigMapEvent` | cluster-agent | ConfigMap created/deleted |
| `SendServicePodMap` | cluster-agent | Bulk pod→service mapping (startup + 30s resync) |
| `SendK8sResourceEvent` | cluster-agent | Generic K8s resource event (Deployment, Ingress, etc.) |
| `SendEnvoyRoutes` | cluster-agent | Domain→service mappings parsed from Envoy ConfigMap |

### Read Path — `QubitQuery`

| RPC | Description |
|-----|-------------|
| `GetTopology` | Full service dependency graph — nodes, upstream, downstream |

## MCP Server

Exposes Qubit's data to AI assistants via the [Model Context Protocol](https://modelcontextprotocol.io) over stdio JSON-RPC.

### Configuration (`mcp-server/config.yaml`)

```yaml
qubit_core:
  grpc_address: "http://localhost:50051"

clickhouse:
  host: localhost
  port: 8123
  user: default
  password: "qubit"
  database: default
  ebpf_table: ebpf_network_events
  k8s_table: k8s_events
```

### Claude Code Integration

The project ships a `.mcp.json` that points Claude Code at the compiled binary:

```bash
cargo build -p qubit-mcp
# Update .mcp.json with the binary path — Claude Code picks it up on next launch.
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `get_topology` | Full service dependency graph |
| `get_service_dependencies` | Upstream/downstream for a single service |
| `get_k8s_events` | Recent Kubernetes resource events |
| `get_network_events` | Raw eBPF-captured HTTP traffic |

## Load Generator

`load-tests` is a simple traffic generator with two configurable streams:

| Stream | What it does | What it exercises |
|--------|-------------|-------------------|
| K8s (`--k8s-rps`) | Creates/patches/deletes ConfigMaps | cluster-agent informers → Core |
| HTTP (`--http-rps`) | GET requests to service-b | eBPF capture → Core |

```bash
# Default: 10 k8s/s + 20 http/s for 60s
make -C ebpf/hack vm-load-gen

# Custom rates
make -C ebpf/hack vm-load-gen K8S_RPS=50 HTTP_RPS=100 DURATION=120

# Or directly from the Lima VM shell
cd load-tests && cargo run --release --bin load-gen -- --k8s-rps 50 --http-rps 100
```

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
| MCP framework | rmcp |
| Dev environment | Lima + Kind |
