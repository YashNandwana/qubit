# ==============================
# eBPF Service Map Development Environment
# ==============================
# Docker-based Linux environment for building and testing eBPF-based service mapping
#
# Build model (mirrors production CI/CD):
#   - eBPF bytecode: compiled via Docker on macOS (platform-independent BPF target)
#   - Loader binary: compiled natively in Lima VM (needs Linux)
#   - Both are packaged into a slim runtime image for Kind/K8s
#
# Quick start:
#   brew install lima
#   make lima-create          # one-time VM with Docker/Kind/kubectl
#   make vm-setup             # one-time Rust install in VM
#   make build-ebpf           # compile eBPF bytecode (Docker, runs on macOS)
#   make vm-test              # cluster + loader build + deploy

# ------------------------------
# Configuration
# ------------------------------
DOCKER_IMAGE ?= ebpf-dev
CONTAINER_NAME ?= ebpf-test
DOCKER ?= docker

# Lima VM Configuration
LIMA_VM ?= qubit
LIMA_CMD = limactl shell $(LIMA_VM) --
PROJECT_DIR = $(shell cd ../.. && pwd)

# Load generator defaults (override on the command line: make vm-load-gen K8S_RPS=50)
K8S_RPS ?= 10
HTTP_RPS ?= 20
DURATION ?= 60

# Kind Configuration
KIND_CLUSTER ?= qubit-test
KIND_IMAGE ?= qubit-ebpf:latest
CLUSTER_AGENT_IMAGE ?= qubit-cluster-agent:latest

# eBPF bytecode output path (built on macOS via Docker)
EBPF_BYTECODE = $(PROJECT_DIR)/ebpf/target/bpfel-unknown-none/release/ebpf

# ------------------------------
# Docker Image Management
# ------------------------------
.PHONY: build-image
build-image:
	@echo "Building eBPF development Docker image..."
	@$(DOCKER) build -t $(DOCKER_IMAGE) -f Dockerfile .

.PHONY: rebuild-image
rebuild-image:
	@echo "Rebuilding eBPF development Docker image (no cache)..."
	@$(DOCKER) build --no-cache -t $(DOCKER_IMAGE) -f Dockerfile .

# ------------------------------
# Container Operations
# ------------------------------
.PHONY: shell
shell: build-image
	@echo "Starting interactive shell in eBPF container..."
	@$(DOCKER) run -it --rm \
		--privileged \
		--name $(CONTAINER_NAME) \
		-v $(shell cd ../.. && pwd):/workspace \
		-w /workspace \
		$(DOCKER_IMAGE) \
		/bin/bash

# ------------------------------
# Build Commands
# ------------------------------

# eBPF bytecode — compiled via Docker (works on macOS, targets bpfel-unknown-none)
# This rarely changes. Rebuild only when you modify ebpf/src/
.PHONY: build-ebpf
build-ebpf: build-image
	@echo "Building eBPF bytecode (via Docker)..."
	@$(DOCKER) run --rm \
		-v $(shell cd ../.. && pwd):/workspace \
		-v ebpf-cargo-registry:/root/.cargo/registry \
		-v ebpf-cargo-git:/root/.cargo/git \
		-w /workspace/ebpf \
		$(DOCKER_IMAGE) \
		/bin/bash -c "cargo build --release"
	@echo "eBPF bytecode built: ebpf/target/bpfel-unknown-none/release/ebpf"

# Loader — compiled via Docker (for local testing only)
.PHONY: build-loader
build-loader: build-image
	@echo "Building eBPF loader (via Docker)..."
	@$(DOCKER) run --rm \
		-v $(shell cd ../.. && pwd):/workspace \
		-v ebpf-cargo-registry:/root/.cargo/registry \
		-v ebpf-cargo-git:/root/.cargo/git \
		-w /workspace/ebpf-loader \
		$(DOCKER_IMAGE) \
		/bin/bash -c "cargo build --release"

.PHONY: build-all
build-all: build-ebpf build-loader
	@echo "All components built successfully!"

# ------------------------------
# Testing - Service Map
# ------------------------------
.PHONY: test-service-map
test-service-map: build-image build-all
	@echo "Running Service Map test..."
	@$(DOCKER) run -it --rm \
		--privileged \
		--name $(CONTAINER_NAME) \
		-v $(shell cd ../.. && pwd):/workspace \
		-w /workspace \
		$(DOCKER_IMAGE) \
		/bin/bash -c '\
			echo "" && \
			RUST_LOG=info INTERFACE=eth0 ./ebpf-loader/target/release/ebpf-loader & \
			PID=$$! && \
			sleep 3 && \
			echo "" && \
			echo "Simulating service-to-service calls..." && \
			echo "" && \
			echo "→ Service A calls service-b.default.svc.cluster.local" && \
			nslookup service-b.default.svc.cluster.local 127.0.0.1 2>/dev/null || true && \
			echo "" && \
			echo "→ Service A calls service-c.payments.svc.cluster.local" && \
			nslookup service-c.payments.svc.cluster.local 127.0.0.1 2>/dev/null || true && \
			echo "" && \
			echo "→ Service A calls external API (api.stripe.com)" && \
			ping -c 1 api.stripe.com 2>/dev/null || true && \
			echo "" && \
			echo "→ Service A calls google.com" && \
			ping -c 1 google.com 2>/dev/null || true && \
			echo "" && \
			echo "→ Service A calls github.com" && \
			curl -s --max-time 3 https://github.com > /dev/null 2>&1 || true && \
			echo "" && \
			sleep 2 && \
			kill -INT $$PID 2>/dev/null && \
			wait $$PID 2>/dev/null || true && \
			echo "" && \
			echo "Service Map test complete!"'

.PHONY: test-dns
test-dns: build-image build-all
	@echo "Running DNS monitoring test..."
	@$(DOCKER) run -it --rm \
		--privileged \
		--name $(CONTAINER_NAME) \
		-v $(shell cd ../.. && pwd):/workspace \
		-w /workspace \
		$(DOCKER_IMAGE) \
		/bin/bash -c '\
			echo "=== DNS Monitor Test ===" && \
			echo "" && \
			echo "Starting monitor in background..." && \
			RUST_LOG=info INTERFACE=eth0 ./ebpf-loader/target/release/ebpf-loader & \
			PID=$$! && \
			sleep 2 && \
			echo "" && \
			echo ">>> Generating DNS traffic..." && \
			echo "" && \
			nslookup google.com 2>/dev/null && \
			sleep 1 && \
			nslookup github.com 2>/dev/null && \
			sleep 1 && \
			nslookup cloudflare.com 2>/dev/null && \
			sleep 2 && \
			echo "" && \
			echo ">>> Stopping monitor and showing summary..." && \
			kill -INT $$PID 2>/dev/null && \
			wait $$PID 2>/dev/null || true && \
			echo "" && \
			echo "Test complete!"'

.PHONY: test-interactive
test-interactive: build-image build-all
	@echo "Running interactive test (Ctrl+C to stop and see summary)..."
	@$(DOCKER) run -it --rm \
		--privileged \
		--name $(CONTAINER_NAME) \
		-v $(shell cd ../.. && pwd):/workspace \
		-w /workspace \
		$(DOCKER_IMAGE) \
		/bin/bash -c 'RUST_LOG=info INTERFACE=eth0 ./ebpf-loader/target/release/ebpf-loader'

# ------------------------------
# Cleanup
# ------------------------------
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf ../ebpf/target ../ebpf-loader/target ../ebpf-common/target

.PHONY: clean-docker
clean-docker:
	@echo "Removing Docker image..."
	@$(DOCKER) rmi $(DOCKER_IMAGE) 2>/dev/null || true

.PHONY: clean-all
clean-all: clean clean-docker
	@echo "All cleaned up!"

# ------------------------------
# Help
# ------------------------------
.PHONY: help
help:
	@echo "eBPF Service Map Development"
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "=== BUILD (macOS) ==="
	@echo "  build-ebpf         Compile eBPF bytecode (via Docker, rarely needed)"
	@echo "  build-image        Build the eBPF dev Docker image"
	@echo "  rebuild-image      Rebuild Docker image (no cache)"
	@echo ""
	@echo "=== LOCAL TESTING (Docker) ==="
	@echo "  build-all          Build eBPF program and loader"
	@echo "  test-service-map   Run service map demo with K8s-style DNS"
	@echo "  test-dns           Run basic DNS monitoring test"
	@echo "  test-interactive   Interactive mode (Ctrl+C for summary)"
	@echo "  shell              Get bash shell in container"
	@echo ""
	@echo "=== LIMA VM LIFECYCLE ==="
	@echo "  lima-create        Create and start the Lima VM (one-time)"
	@echo "  lima-start         Start an existing Lima VM"
	@echo "  lima-stop          Stop the Lima VM"
	@echo "  lima-delete        Delete the Lima VM entirely"
	@echo "  lima-status        Show Lima VM status"
	@echo ""
	@echo "=== LIMA VM TESTING (Kind/K8s - Production-like) ==="
	@echo "  vm-setup                    Install Rust in VM (one-time)"
	@echo "  vm-check                    Check Lima VM connectivity and tools"
	@echo "  vm-cluster-create           Create Kind cluster in Lima VM"
	@echo "  vm-cluster-delete           Delete Kind cluster in Lima VM"
	@echo "  vm-build-loader             Build loader natively in Lima VM"
	@echo "  vm-cluster-agent            Build + package cluster-agent for Kind"
	@echo "  vm-docker-build             Build eBPF K8s runtime image"
	@echo "  vm-kind-load                Load eBPF image into Kind cluster"
	@echo "  vm-envoy-pull               Pull Envoy image from Docker Hub and load into Kind (run once)"
	@echo "  vm-deploy                   Deploy cluster-agent + envoy-proxy + DaemonSet + test pods"
	@echo "  vm-logs                     View eBPF DaemonSet logs"
	@echo "  vm-status                   Check pod status"
	@echo "  vm-test                     Full end-to-end test (first time)"
	@echo "  vm-redeploy                 Rebuild all + redeploy (fast iteration)"
	@echo "  vm-chaos                    Chaos loop: create/update/delete objects"
	@echo "  vm-cleanup                  Remove all deployments"
	@echo "  vm-shell                    Open a shell in the Lima VM"
	@echo ""
	@echo "=== LOAD GENERATOR (Natural Traffic) ==="
	@echo "  vm-load-gen                 Full pipeline: build + package + deploy"
	@echo "  vm-load-gen-deploy          Deploy load-gen Job (skips build)"
	@echo "  vm-load-gen-logs            Follow load-gen output"
	@echo "  vm-load-gen-status          Check Job and pod status"
	@echo "  vm-load-gen-cleanup         Remove Job and load-gen namespace"
	@echo ""
	@echo "Cleanup:"
	@echo "  clean              Remove build artifacts"
	@echo "  clean-docker       Remove Docker image"
	@echo "  clean-all          Remove everything"

# ==============================
# LIMA VM LIFECYCLE
# ==============================

# ------------------------------
# Lima VM Management
# ------------------------------
.PHONY: lima-create
lima-create:
	@echo "Creating and starting Lima VM '$(LIMA_VM)'..."
	limactl create --name=$(LIMA_VM) ../../lima.yaml
	limactl start $(LIMA_VM)
	@echo "Lima VM '$(LIMA_VM)' is ready. Next: make vm-setup"

.PHONY: lima-start
lima-start:
	@echo "Starting Lima VM '$(LIMA_VM)'..."
	limactl start $(LIMA_VM)

.PHONY: lima-stop
lima-stop:
	@echo "Stopping Lima VM '$(LIMA_VM)'..."
	limactl stop $(LIMA_VM)

.PHONY: lima-delete
lima-delete:
	@echo "Deleting Lima VM '$(LIMA_VM)'..."
	limactl delete --force $(LIMA_VM)

.PHONY: lima-status
lima-status:
	@echo "Lima VM status:"
	limactl list

# ==============================
# VM-BASED TESTING (Production-like)
# ==============================
# Uses Lima VM with Kind cluster for production-like testing
#
# Build model:
#   eBPF bytecode = compiled on macOS via Docker (make build-ebpf)
#   Loader binary = compiled natively in Lima VM (make vm-build-loader)

# ------------------------------
# VM Toolchain Setup (one-time)
# ------------------------------
.PHONY: vm-setup
vm-setup:
	@echo "Installing Rust toolchain in Lima VM (loader only, no bpf-linker)..."
	@$(LIMA_CMD) bash -c '\
		if command -v rustup &>/dev/null; then \
			echo "Rust already installed, skipping."; \
		else \
			curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable; \
		fi && \
		export PATH="$$HOME/.cargo/bin:$$PATH" && \
		rustc --version && \
		echo "Setup complete! Run: make build-ebpf && make vm-test"'

# ------------------------------
# VM Connectivity
# ------------------------------
.PHONY: vm-check
vm-check:
	@echo "Checking Lima VM connectivity..."
	@$(LIMA_CMD) bash -c "echo 'Connected to Lima VM!' && docker --version && kubectl version --client --short 2>/dev/null || kubectl version --client && kind --version && rustc --version 2>/dev/null || echo 'Rust not installed — run: make vm-setup'"

.PHONY: vm-shell
vm-shell:
	@echo "Opening shell in Lima VM..."
	limactl shell $(LIMA_VM)

# ------------------------------
# Kind Cluster Management
# ------------------------------
.PHONY: vm-cluster-create
vm-cluster-create:
	@echo "Creating Kind cluster '$(KIND_CLUSTER)' in Lima VM..."
	@$(LIMA_CMD) bash -c "kind create cluster --name $(KIND_CLUSTER) --wait 60s || echo 'Cluster may already exist'; \
		kubectl cluster-info --context kind-$(KIND_CLUSTER)"

.PHONY: vm-cluster-delete
vm-cluster-delete:
	@echo "Deleting Kind cluster '$(KIND_CLUSTER)' in Lima VM..."
	@$(LIMA_CMD) bash -c "kind delete cluster --name $(KIND_CLUSTER)"

.PHONY: vm-cluster-status
vm-cluster-status:
	@echo "Kind cluster status..."
	@$(LIMA_CMD) bash -c "kind get clusters && kubectl get nodes"

# ------------------------------
# Build
# ------------------------------

# Build loader natively in VM (fast — only recompiles changed code)
.PHONY: vm-build-loader
vm-build-loader:
	@echo "Building loader in Lima VM..."
	@$(LIMA_CMD) bash -c "export PATH=\$$HOME/.cargo/bin:\$$PATH && cd $(PROJECT_DIR)/ebpf-loader && cargo build --release"
	@echo "Loader built."

# Build K8s runtime image (uses pre-built bytecode + loader)
.PHONY: vm-docker-build
vm-docker-build:
	@echo "Checking for eBPF bytecode..."
	@test -f $(EBPF_BYTECODE) || { echo "ERROR: eBPF bytecode not found. Run 'make build-ebpf' first."; exit 1; }
	@echo "Building K8s runtime image in Lima VM..."
	@$(LIMA_CMD) bash -c "cd $(PROJECT_DIR) && docker build -t $(KIND_IMAGE) -f ebpf/hack/Dockerfile.k8s ."
	@echo "Docker image built: $(KIND_IMAGE)"

.PHONY: vm-kind-load
vm-kind-load:
	@echo "Loading image into Kind cluster..."
	@$(LIMA_CMD) bash -c "kind load docker-image $(KIND_IMAGE) --name $(KIND_CLUSTER)"
	@echo "Image loaded into Kind cluster"

# ------------------------------
# Cluster Agent (Build & Deploy)
# ------------------------------

# Build cluster-agent binary natively in Lima VM
.PHONY: vm-build-cluster-agent
vm-build-cluster-agent:
	@echo "Building cluster-agent in Lima VM..."
	@$(LIMA_CMD) bash -c "export PATH=\$$HOME/.cargo/bin:\$$PATH && cd $(PROJECT_DIR)/cluster-agent && cargo build --release"
	@echo "Cluster-agent built."

# Build cluster-agent Docker image in Lima VM
.PHONY: vm-docker-build-cluster-agent
vm-docker-build-cluster-agent:
	@echo "Building cluster-agent K8s image in Lima VM..."
	@$(LIMA_CMD) bash -c "cd $(PROJECT_DIR) && docker build -t $(CLUSTER_AGENT_IMAGE) -f ebpf/hack/Dockerfile.cluster-agent ."
	@echo "Docker image built: $(CLUSTER_AGENT_IMAGE)"

# Load cluster-agent image into Kind cluster
.PHONY: vm-kind-load-cluster-agent
vm-kind-load-cluster-agent:
	@echo "Loading cluster-agent image into Kind cluster..."
	@$(LIMA_CMD) bash -c "kind load docker-image $(CLUSTER_AGENT_IMAGE) --name $(KIND_CLUSTER)"
	@echo "Cluster-agent image loaded into Kind cluster"

# Full cluster-agent pipeline: build, package, load
.PHONY: vm-cluster-agent
vm-cluster-agent: vm-build-cluster-agent vm-docker-build-cluster-agent vm-kind-load-cluster-agent
	@echo "Cluster-agent ready for deployment."

# ------------------------------
# Kubernetes Deployment
# ------------------------------
# Pull the Envoy image into the Lima VM and load it into Kind so the pod
# doesn't need to hit Docker Hub at runtime (avoids ImagePullBackOff).
# Run this once after vm-cluster-create, before vm-deploy.
# kind load docker-image fails on multi-platform manifest images because it
# tries to import all architectures but only the host platform is pulled locally.
# docker save exports only the pulled platform → kind load image-archive works.
.PHONY: vm-envoy-pull
vm-envoy-pull:
	@echo "Pulling Envoy image into Lima VM..."
	@$(LIMA_CMD) bash -c "docker pull envoyproxy/envoy:v1.29-latest"
	@echo "Loading Envoy image into Kind cluster (via image-archive to avoid multi-platform issue)..."
	@$(LIMA_CMD) bash -c "docker save envoyproxy/envoy:v1.29-latest -o /tmp/envoy-kind.tar && \
		kind load image-archive /tmp/envoy-kind.tar --name $(KIND_CLUSTER) && \
		rm -f /tmp/envoy-kind.tar"
	@echo "Envoy image ready in Kind."

.PHONY: vm-deploy
vm-deploy:
	@echo "Deploying cluster-agent, envoy-proxy, test pods, and eBPF DaemonSet..."
	@$(LIMA_CMD) bash -c "kubectl apply -f $(PROJECT_DIR)/ebpf/hack/k8s/cluster-agent.yaml && \
		kubectl apply -f $(PROJECT_DIR)/ebpf/hack/k8s/envoy-proxy.yaml && \
		kubectl apply -f $(PROJECT_DIR)/ebpf/hack/k8s/test-pods.yaml && \
		kubectl apply -f $(PROJECT_DIR)/ebpf/hack/k8s/ebpf-daemonset.yaml"
	@echo "Waiting for pods to be ready..."
	@$(LIMA_CMD) bash -c "kubectl wait --for=condition=Ready pod -l app=qubit-ebpf --timeout=60s 2>/dev/null || true && \
		kubectl wait --for=condition=Ready pod -l app=qubit-cluster-agent --timeout=60s 2>/dev/null || true && \
		kubectl wait --for=condition=Ready pod -l app=envoy-proxy --timeout=120s 2>/dev/null || true"
	@echo "Deployed! Use 'make vm-status' to check status"

.PHONY: vm-status
vm-status:
	@echo "Pod status..."
	@$(LIMA_CMD) bash -c "kubectl get pods -o wide"

.PHONY: vm-logs
vm-logs:
	@echo "eBPF DaemonSet logs..."
	@$(LIMA_CMD) bash -c "kubectl logs -l app=qubit-ebpf --tail=50 -f" || true

.PHONY: vm-logs-pods
vm-logs-pods:
	@echo "Test pod logs..."
	@$(LIMA_CMD) bash -c "kubectl logs -l app=service-a --tail=20; \
		kubectl logs -l app=service-b --tail=20"

.PHONY: vm-cleanup
vm-cleanup:
	@echo "Removing deployments..."
	@$(LIMA_CMD) bash -c "kubectl delete -f $(PROJECT_DIR)/ebpf/hack/k8s/ebpf-daemonset.yaml --ignore-not-found && \
		kubectl delete -f $(PROJECT_DIR)/ebpf/hack/k8s/test-pods.yaml --ignore-not-found && \
		kubectl delete -f $(PROJECT_DIR)/ebpf/hack/k8s/envoy-proxy.yaml --ignore-not-found && \
		kubectl delete -f $(PROJECT_DIR)/ebpf/hack/k8s/cluster-agent.yaml --ignore-not-found"
	@echo "Cleanup complete"

# ------------------------------
# Chaos Testing (exercises cluster-agent inside the VM cluster)
# ------------------------------
CHAOS_NAMESPACE ?= playground
CHAOS_MANIFESTS = $(PROJECT_DIR)/core/hack/manifests

.PHONY: vm-chaos-create
vm-chaos-create:
	@$(LIMA_CMD) bash -c "kubectl create namespace $(CHAOS_NAMESPACE) --dry-run=client -o yaml | kubectl apply -f - && \
		kubectl apply -f $(CHAOS_MANIFESTS)/configmap.yaml -n $(CHAOS_NAMESPACE) && \
		kubectl apply -f $(CHAOS_MANIFESTS)/service.yaml -n $(CHAOS_NAMESPACE) && \
		kubectl apply -f $(CHAOS_MANIFESTS)/pod.yaml -n $(CHAOS_NAMESPACE) && \
		kubectl apply -f $(CHAOS_MANIFESTS)/deployment.yaml -n $(CHAOS_NAMESPACE) && \
		kubectl apply -f $(CHAOS_MANIFESTS)/hpa.yaml -n $(CHAOS_NAMESPACE)"

.PHONY: vm-chaos-update
vm-chaos-update:
	@$(LIMA_CMD) bash -c "\
		kubectl patch configmap sample-config -n $(CHAOS_NAMESPACE) --type merge -p '{\"data\":{\"UPDATED_KEY\":\"updated-value\"}}' || true && \
		kubectl patch service sample-service -n $(CHAOS_NAMESPACE) --type merge -p '{\"spec\":{\"ports\":[{\"port\":8080,\"targetPort\":80}]}}' || true && \
		kubectl patch deployment sample-deployment -n $(CHAOS_NAMESPACE) --type merge -p '{\"spec\":{\"replicas\":3}}' || true"

.PHONY: vm-chaos-delete
vm-chaos-delete:
	@$(LIMA_CMD) bash -c "\
		kubectl delete -f $(CHAOS_MANIFESTS)/hpa.yaml -n $(CHAOS_NAMESPACE) --ignore-not-found && \
		kubectl delete -f $(CHAOS_MANIFESTS)/deployment.yaml -n $(CHAOS_NAMESPACE) --ignore-not-found && \
		kubectl delete -f $(CHAOS_MANIFESTS)/configmap.yaml -n $(CHAOS_NAMESPACE) --ignore-not-found && \
		kubectl delete -f $(CHAOS_MANIFESTS)/service.yaml -n $(CHAOS_NAMESPACE) --ignore-not-found && \
		kubectl delete -f $(CHAOS_MANIFESTS)/pod.yaml -n $(CHAOS_NAMESPACE) --ignore-not-found"

.PHONY: vm-chaos
vm-chaos:
	@echo "Starting chaos loop in Lima VM cluster (ctrl-c to stop)..."
	@while true; do \
		$(MAKE) vm-chaos-create; \
		sleep 30; \
		$(MAKE) vm-chaos-update; \
		sleep 30; \
		$(MAKE) vm-chaos-delete; \
		sleep 30; \
	done

# ------------------------------
# Full Test Workflow
# ------------------------------

# First-time: cluster + build everything + deploy
.PHONY: vm-test
vm-test: vm-cluster-create vm-build-loader vm-cluster-agent vm-docker-build vm-kind-load vm-envoy-pull vm-deploy
	@echo ""
	@echo "=========================================="
	@echo "  Production-like test environment ready!"
	@echo "=========================================="
	@echo ""
	@echo "Next steps:"
	@echo "  make vm-status   - Check pod status"
	@echo "  make vm-logs     - View eBPF captured events"
	@echo "  make vm-cleanup  - Remove deployments"
	@echo ""

# Fast iteration: rebuild loader + cluster-agent + redeploy (skips cluster + eBPF bytecode)
.PHONY: vm-redeploy
vm-redeploy: vm-build-loader vm-cluster-agent vm-docker-build vm-kind-load vm-cleanup vm-deploy
	@echo ""
	@echo "Redeployed! Use 'make vm-logs' to check output."
	@echo ""

# ------------------------------
# Load generator
# Override defaults: make vm-load-gen K8S_RPS=50 HTTP_RPS=100 DURATION=120
# ------------------------------
.PHONY: vm-load-gen
vm-load-gen:
	@$(LIMA_CMD) bash -c "export PATH=\$$HOME/.cargo/bin:\$$PATH && \
		cd $(PROJECT_DIR)/load-tests && \
		K8S_RPS=$(K8S_RPS) HTTP_RPS=$(HTTP_RPS) DURATION=$(DURATION) \
		cargo run --release --bin load-gen"
