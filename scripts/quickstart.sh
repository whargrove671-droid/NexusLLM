#!/bin/bash
# NexusLLM Quick Start Script
# This script sets up a local development environment

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🚀 NexusLLM Quick Start"
echo "========================"

# Check prerequisites
check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo "❌ $1 is not installed. Please install it first."
        exit 1
    fi
    echo "✅ $1 found"
}

echo "Checking prerequisites..."
check_command "docker"
check_command "docker-compose"
check_command "rustc"
check_command "cargo"
check_command "protoc"

# Check for GPU support
if command -v nvidia-smi &> /dev/null; then
    echo "✅ NVIDIA GPU detected"
    GPU_AVAILABLE=true
else
    echo "⚠️  No NVIDIA GPU detected, will use CPU mode"
    GPU_AVAILABLE=false
fi

# Create config directory
mkdir -p "$PROJECT_ROOT/config"

# Generate worker config if not exists
if [ ! -f "$PROJECT_ROOT/config/worker.toml" ]; then
    cat > "$PROJECT_ROOT/config/worker.toml" << 'EOF'
orchestrator_url = "ws://localhost:8080"
model_cache_dir = "./model_cache"
tee_enabled = false
heartbeat_interval_secs = 10
attestation_interval_secs = 300
max_concurrent_tasks = 1
log_level = "debug"
metrics_port = 9090

[stake_info]
worker_address = ""
delegated_address = ""
self_stake = 0
delegated_stake = 0
total_stake = 0
locked_until = 0

[metadata]
operator = "local-dev"
location = "localhost"
EOF
    echo "✅ Created config/worker.toml"
fi

# Start infrastructure
echo ""
echo "Starting infrastructure services..."
cd "$PROJECT_ROOT/docker"
docker-compose up -d postgres redis ipfs jaeger prometheus grafana

# Wait for services to be healthy
echo "Waiting for services to be ready..."
sleep 10

# Build the project
echo ""
echo "Building NexusLLM..."
cd "$PROJECT_ROOT"

# Build protobuf first
cd crates/nexus-proto
cargo build
cd ../..

# Build the agent
if [ "$GPU_AVAILABLE" = true ]; then
    cargo build --release --bin nexus-agent --features cuda
else
    cargo build --release --bin nexus-agent
fi

echo ""
echo "✅ Build complete!"
echo ""
echo "To start the orchestrator services:"
echo "  cd docker && docker-compose up -d api-gateway worker-registry model-registry scheduler settlement"
echo ""
echo "To start a worker node:"
echo "  ./target/release/nexus-agent --config ../config/worker.toml"
echo ""
echo "To submit an inference request:"
echo "  curl -X POST http://localhost:8080/v1/inference \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\": \"llama-3-8b-instruct\", \"prompt\": \"Hello, distributed world!\", \"stream\": true}'"
echo ""
echo "📊 Grafana: http://localhost:3000 (admin/admin)"
echo "📈 Prometheus: http://localhost:9090"
echo "🔍 Jaeger: http://localhost:16686"
echo ""
echo "Happy distributed inferencing! 🎉"