# ==============================
# Kubernetes Playground Makefile
# ==============================
CLUSTER_NAME ?= local-cluster
NAMESPACE ?= playground
KUBECTL ?= kubectl
KIND ?= kind

REPO_ROOT := $(shell cd ../.. && pwd)

# ------------------------------
# Helpers
# ------------------------------
has-external-secret-api:
	@$(KUBECTL) api-resources --api-group=external-secrets.io >/dev/null 2>&1 && echo "yes" || echo "no"

# ------------------------------
# Cluster Setup
# ------------------------------
.PHONY: cluster-up
cluster-up:
	@echo "🚀 Starting Kind cluster: $(CLUSTER_NAME)"
	@if ! $(KIND) get clusters | grep -q $(CLUSTER_NAME); then \
		$(KIND) create cluster --name $(CLUSTER_NAME); \
	else \
		echo "Cluster $(CLUSTER_NAME) already exists."; \
	fi
	@echo "✅ Creating namespace $(NAMESPACE) (if missing)"
	@$(KUBECTL) create namespace $(NAMESPACE) --dry-run=client -o yaml | $(KUBECTL) apply -f -

.PHONY: cluster-down
cluster-down:
	@echo "🧹 Deleting Kind cluster: $(CLUSTER_NAME)"
	@$(KIND) delete cluster --name $(CLUSTER_NAME)

# ------------------------------
# Optional: Install External Secrets Operator
# ------------------------------
.PHONY: install-external-secrets
install-external-secrets:
	@echo "ℹ️  To install External Secrets Operator (ESO) follow its official install instructions:"
	@echo "   - Visit: https://github.com/external-secrets/external-secrets"
	@echo "   - Typical install (example):"
	@echo "       kubectl apply -f https://github.com/external-secrets/external-secrets/releases/latest/download/crds.yaml"
	@echo "       kubectl apply -f https://github.com/external-secrets/external-secrets/releases/latest/download/operator.yaml"
	@echo "   (Run above as cluster-admin / with network access.)"

# ------------------------------
# Objects Setup (safe apply)
# ------------------------------
.PHONY: create-objects
create-objects:
	@echo "📦 Creating K8s native objects in namespace $(NAMESPACE)"
	@$(KUBECTL) apply -f manifests/configmap.yaml -n $(NAMESPACE)
	@$(KUBECTL) apply -f manifests/service.yaml -n $(NAMESPACE)
	@$(KUBECTL) apply -f manifests/pod.yaml -n $(NAMESPACE)
	@echo "🔎 Checking for ExternalSecret API..."
	@if [ "$$($(KUBECTL) api-resources --api-group=external-secrets.io 2>/dev/null | grep -c ExternalSecret)" -ge 1 ]; then \
		echo "✅ ExternalSecret API present — applying external secret"; \
		$(KUBECTL) apply -f manifests/external-secret.yaml -n $(NAMESPACE); \
	else \
		echo "⚠️  ExternalSecret API not found — skipping manifests/external-secret.yaml (install operator with 'make install-external-secrets' if you want it)"; \
	fi

.PHONY: update-objects
update-objects:
	@echo "🔁 Updating ConfigMap and Service"
	@$(KUBECTL) patch configmap sample-config -n $(NAMESPACE) --type merge -p '{"data":{"UPDATED_KEY":"updated-value"}}' || true
	@$(KUBECTL) patch service sample-service -n $(NAMESPACE) --type merge -p '{"spec":{"ports":[{"port":8080,"targetPort":80}]}}' || true

.PHONY: delete-objects
delete-objects:
	@echo "🗑️  Deleting K8s objects"
	@$(KUBECTL) delete -f manifests/configmap.yaml -n $(NAMESPACE) --ignore-not-found || true
	@$(KUBECTL) delete -f manifests/service.yaml -n $(NAMESPACE) --ignore-not-found || true
	@$(KUBECTL) delete -f manifests/pod.yaml -n $(NAMESPACE) --ignore-not-found || true
	@if [ "$$($(KUBECTL) api-resources --api-group=external-secrets.io 2>/dev/null | grep -c ExternalSecret)" -ge 1 ]; then \
		$(KUBECTL) delete -f manifests/external-secret.yaml -n $(NAMESPACE) --ignore-not-found || true; \
	else \
		echo "⚠️  ExternalSecret API not found — skipping delete for external secret"; \
	fi

# ------------------------------
# Continuous Update Loop
# ------------------------------
.PHONY: chaos
chaos:
	@echo "🔥 Starting chaos loop — create, update, delete repeatedly (ctrl-c to stop)"
	@while true; do \
		$(MAKE) create-objects; \
		sleep 30; \
		$(MAKE) update-objects; \
		sleep 30; \
		$(MAKE) delete-objects; \
		sleep 30; \
	done

# ------------------------------
# Server & DB
# ------------------------------
.PHONY: core-up
core-up:
	@echo "🚀 Starting ClickHouse DB"
	@docker compose -f clickhouse/docker-compose.yaml up -d
	@echo "⏳ Waiting for ClickHouse to be ready..."
	@for i in 1 2 3 4 5 6 7 8 9 10; do \
		docker exec $$(docker compose -f clickhouse/docker-compose.yaml ps -q) clickhouse-client --query "SELECT 1" >/dev/null 2>&1 && break; \
		echo "  Waiting... ($$i/10)"; \
		sleep 1; \
	done
	@echo "🦀 Starting Qubit core..."
	@cd .. && cargo run

.PHONY: core-down
core-down:
	@echo "🛑 Stopping Qubit core & DB"
	@docker compose -f clickhouse/docker-compose.yaml down

# ------------------------------
# Cluster Agent
# ------------------------------
.PHONY: cluster-agent-up
cluster-agent-up:
	@echo "🤖 Starting cluster-agent..."
	@cd $(REPO_ROOT)/cluster-agent && CONFIG_PATH=config.yaml cargo run

.PHONY: cluster-agent-down
cluster-agent-down:
	@echo "🛑 Stopping cluster-agent"
	@pkill -f "cluster-agent" || true
