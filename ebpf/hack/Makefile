# ==============================
# eBPF Service Map Development Environment
# ==============================
# Docker-based Linux environment for building and testing eBPF-based service mapping

DOCKER_IMAGE ?= ebpf-dev
CONTAINER_NAME ?= ebpf-test
DOCKER ?= docker

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
.PHONY: build-ebpf
build-ebpf: build-image
	@echo "Building eBPF program..."
	@$(DOCKER) run --rm \
		-v $(shell cd ../.. && pwd):/workspace \
		-w /workspace/ebpf \
		$(DOCKER_IMAGE) \
		/bin/bash -c "cargo build --release"

.PHONY: build-loader
build-loader: build-image
	@echo "Building eBPF loader..."
	@$(DOCKER) run --rm \
		-v $(shell cd ../.. && pwd):/workspace \
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
			echo "✅ Service Map test complete!"'

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
	@echo "🧹 Cleaning build artifacts..."
	@rm -rf ../ebpf/target ../ebpf-loader/target ../ebpf-common/target

.PHONY: clean-docker
clean-docker:
	@echo "🧹 Removing Docker image..."
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
	@echo "Build:"
	@echo "  build-image        Build the Docker image"
	@echo "  rebuild-image      Rebuild Docker image (no cache)"
	@echo "  build-all          Build eBPF program and loader"
	@echo ""
	@echo "Test:"
	@echo "  test-service-map   Run service map demo with K8s-style DNS"
	@echo "  test-dns           Run basic DNS monitoring test"
	@echo "  test-interactive   Interactive mode (Ctrl+C for summary)"
	@echo "  shell              Get bash shell in container"
	@echo ""
	@echo "Cleanup:"
	@echo "  clean              Remove build artifacts"
	@echo "  clean-docker       Remove Docker image"
	@echo "  clean-all          Remove everything"
