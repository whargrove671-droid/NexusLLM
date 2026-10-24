# NexusLLM — Complete Project Documentation

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Core Components](#core-components)
4. [Installation & Deployment](#installation--deployment)
5. [Configuration](#configuration)
6. [API Reference](#api-reference)
7. [Worker Node Operations](#worker-node-operations)
8. [Orchestrator Operations](#orchestrator-operations)
9. [Model Sharding & Pipeline](#model-sharding--pipeline)
10. [Security & TEE](#security--tee)
11. [Economics & Settlement](#economics--settlement)
12. [Monitoring & Observability](#monitoring--observability)
13. [Development Guide](#development-guide)
14. [Troubleshooting](#troubleshooting)
15. [License](#license)

---

## Overview

**NexusLLM** transforms the world's idle compute into a planetary-scale inference fabric. It aggregates heterogeneous compute resources (home desktops, laptops, mobile devices) into a unified inference cluster capable of running large language models (7B–400B+ parameters) with performance approaching dedicated data-center deployments.

### Key Differentiators

| Feature | Description |
|---------|-------------|
| **Heterogeneous-First** | Native CUDA, ROCm, Metal/MLX, Intel AMX, ARM NEON, WASM |
| **TEE-Secured** | Hardware attestation (SGX/SEV/TrustZone/Secure Enclave) |
| **P2P Data Plane** | libp2p + WebRTC mesh eliminates central bandwidth bottlenecks |
| **Dynamic Sharding** | Pipeline + tensor + expert parallelism with runtime rebalancing |
| **Crypto-Economic** | ZK proof-of-compute, stake-weighted sybil resistance |

### Supported Models

| Model Family | Parameters | Min VRAM | Recommended Hardware |
|-------------|------------|----------|---------------------|
| LLaMA-3 | 8B, 70B | 8GB / 48GB | RTX 3090/4090, A100, H100 |
| Mistral | 7B, 8x7B (MoE) | 8GB / 48GB | RTX 3090/4090, A100 |
| Mixtral | 8x7B, 8x22B | 48GB / 128GB | A100, H100, multi-GPU |
| Gemma | 2B, 7B | 2GB / 8GB | RTX 3060+, M-series |
| Qwen | 7B, 14B, 72B | 8GB / 48GB | RTX 3090/4090, A100 |
| Phi-3 | 3.8B, 14B | 4GB / 16GB | RTX 3060+, M-series |

### Hardware Backends

| Platform | Backend | Quantization | Status |
|----------|---------|--------------|--------|
| NVIDIA GPU | CUDA + FlashAttention-2 | FP16, BF16, FP8, INT8, GPTQ, AWQ | ✅ Production |
| AMD GPU | ROCm + FlashAttention | FP16, BF16, INT8 | ✅ Beta |
| Apple Silicon | Metal + MLX | FP16, INT4, INT8 | ✅ Production |
| Intel CPU | oneDNN + AMX + llama.cpp | GGUF 4-bit, Q8_0 | ✅ Production |
| ARM CPU | llama.cpp + NEON | GGUF 4-bit | ✅ Beta |
| Browser | WebGPU + WebLLM | INT4 | 🚧 Experimental |
| Mobile | ExecuTorch / CoreML | INT4/INT8 | 🚧 Experimental |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        NEXUS ORCHESTRATOR LAYER (Cloud)                     │
│  ┌──────────┐  ┌────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │API Gateway│→│Task Scheduler│→│Model Registry│→│Worker Registry     │   │
│  └──────────┘  └────────────┘  └──────────────┘  └────────────────────┘   │
│        │              │                │                    │              │
│        └──────────────┴────────────────┴────────────────────┘              │
│                               │                                           │
│                    ┌──────────▼──────────┐                                │
│                    │  Message Bus        │                                │
│                    │  (NATS/Kafka)       │                                │
│                    └──────────┬──────────┘                                │
└───────────────────────────────│───────────────────────────────────────────┘
                                │ P2P (libp2p + WebRTC)
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│ Worker Node 1 │      │ Worker Node 2 │      │ Worker Node N │
│ RTX 4090      │      │ RTX 3080      │      │ M3 Max / CPU  │
│ ┌───────────┐ │      │ ┌───────────┐ │      │ ┌───────────┐ │
│ │Nexus Agent│ │      │ │Nexus Agent│ │      │ │Nexus Agent│ │
│ ├───────────┤ │      │ ├───────────┤ │      │ ├───────────┤ │
│ │Shard Exec │ │      │ │Shard Exec │ │      │ │Shard Exec │ │
│ ├───────────┤ │      │ ├───────────┤ │      │ ├───────────┤ │
│ │Model Cache│ │      │ │Model Cache│ │      │ │Model Cache│ │
│ ├───────────┤ │      │ ├───────────┤ │      │ ├───────────┤ │
│ │TEE/SGX    │ │      │ │TEE/SGX    │ │      │ │Secure Encl.│ │
│ └───────────┘ │      │ └───────────┘ │      │ └───────────┘ │
└───────────────┘      └───────────────┘      └───────────────┘
```

### Data Flow

1. **Client** submits inference request via REST/WebSocket
2. **API Gateway** authenticates, rate-limits, forwards to Scheduler
3. **Scheduler** builds DAG, assigns shards to eligible workers
4. **Workers** fetch shard manifests, download weights (IPFS/CDN)
5. **Pipeline Execution** with KV cache transfer over WebRTC
6. **Results** aggregated, ZK proof generated, submitted to Settlement
7. **Rewards** distributed, metrics emitted to Observability stack

---

## Core Components

### 1. Nexus Agent (Worker Daemon)

**Location**: `crates/nexus-agent/`

The agent runs on each worker node and manages:
- Hardware identity derivation (TPM-bound)
- Capability discovery (GPU/CPU, VRAM, TEE availability)
- Task lifecycle: pull → verify → execute → prove → submit
- Heartbeat emission (every 10s via GossipSub)
- Model cache management (4-tier hierarchy)
- TEE attestation refresh (every 5 min)

**Key Files**:
- `src/lib.rs` — Main agent loop, registration, status
- `src/capabilities.rs` — Hardware discovery, peer ID generation
- `src/task_runner.rs` — Task execution with TEE support
- `src/heartbeat.rs` — Periodic health/status reporting
- `src/attestation.rs` — TEE quote generation/verification

**Run**:
```bash
nexus-agent --config config/worker.toml
```

### 2. Shard Executor Trait & Backends

**Location**: `crates/nexus-executor/`

Unified interface for model shard execution across heterogeneous hardware:

```rust
#[async_trait]
pub trait ShardExecutor: Send + Sync {
    fn backend_name(&self) -> &'static str;
    async fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()>;
    async fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch>;
    fn get_kv_cache(&self) -> KVCacheHandle;
    fn set_kv_cache(&mut self, cache: KVCacheHandle);
    fn memory_usage(&self) -> MemoryStats;
    async fn warmup(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}
```

**Backends**:
| Backend | File | Hardware | Features |
|---------|------|----------|----------|
| CUDA | `cuda-backend/` | NVIDIA GPU | FlashAttention-2, FP8, tensor parallel |
| Metal | `metal-backend/` | Apple Silicon | MLX integration, unified memory |
| ROCm | `rocm-backend/` | AMD GPU | FlashAttention, HIP kernels |
| llama.cpp | `llama-cpp-backend/` | CPU (all) | GGUF quantization, AVX2/AMX/NEON |
| WASM | `wasm-backend/` | Browser/Edge | WebGPU, WASM SIMD |

### 3. TEE Abstraction Layer

**Location**: `crates/nexus-tee/`

Unified interface for hardware attestation across platforms:

| TEE | Platform | Attestation | Sealing | Key Derivation |
|-----|----------|-------------|---------|----------------|
| SGX | Intel x86 | DCAP Quote | EPC Sealing | ECDH in EPC |
| SEV-SNP | AMD x86 | SNP Report | Memory Encryption | VMPCK |
| TrustZone | ARM | TEE Gratis | Secure Storage | TEE Internal |
| Secure Enclave | Apple | App Attest | Keychain | Secure Enclave P256 |
| Software | Any | None | AES-GCM | HKDF |

**Security Properties**:
- Model weights never in host memory plaintext
- Input/output encryption via TEE-derived session keys
- ZK proof of execution trace inside TEE
- Remote attestation verified by orchestrator

### 4. P2P Networking Layer

**Location**: `crates/nexus-p2p/`

Built on libp2p with:
- **Transport**: QUIC (primary), WebSocket, WebRTC, TCP
- **NAT Traversal**: ICE + STUN + TURN (incentivized relays)
- **Discovery**: mDNS (LAN), Kademlia DHT (WAN), bootstrap nodes
- **PubSub**: GossipSub for heartbeats, task announcements, reshards
- **Data Channels**: WebRTC DataChannels (ordered, reliable) for KV cache

**Mesh Topology**:
```
Coordinators (backbone, staked) ←→ Workers (leaves, 3-5 connections)
                                    ↳ Edge/Mobile (relay-assisted)
```

### 5. Task Scheduler

**Location**: `crates/nexus-scheduler/`

Heterogeneous DAG planner with cost model:

```python
def plan(request, workers, model):
    strategy = choose_strategy(model, workers, request)
    assignments = assign_shards(model.shards, workers, strategy)
    dag = build_dag(assignments, strategy)
    return ExecutionPlan(dag, latency_estimate, cost_estimate)
```

**Strategies**:
| Model Type | Primary | Fallback |
|------------|---------|----------|
| Dense (LLaMA) | Pipeline | Tensor (if >8 GPUs) |
| MoE (Mixtral) | Expert + Pipeline | Tensor |
| Small (<7B) | Single + Speculative | Pipeline |
| Long Context | Ring Attention + KV Offload | Pipeline |

### 6. Model Registry

**Location**: `crates/nexus-registry/`

Content-addressable storage with:
- **Backend**: IPFS/Filecoin + CDN cache
- **Manifests**: Versioned, Merkle-verified, delta-updated (bsdiff)
- **Compatibility Matrix**: Per-shard backend support, VRAM requirements

### 7. Settlement Engine

**Location**: `crates/nexus-settlement/`

On-chain (Arbitrum/Solana) + off-chain:
- **Staking**: `stake(worker, amount)` — min 10k NEXUS
- **Delegation**: `delegate(staker, operator, amount)` — liquid stNEXUS
- **Rewards**: Pro-rata by compute contribution × quality multiplier
- **Slashing**: Failed attestation (10%), incorrect output (25-50%), downtime
- **ZK Verification**: RISC Zero / SP1 proofs on-chain (~5-10ms)

---

## Installation & Deployment

### Prerequisites

- Docker & Docker Compose
- Rust 1.78+
- protoc (Protocol Buffers compiler) — v36.2+
- NVIDIA GPU + CUDA 12.3+ (for CUDA backend)
- 16GB+ RAM, 100GB+ disk for model cache

### Quick Start (Local Development)

```bash
# Clone and build
git clone https://github.com/nexusllm/nexusllm
cd nexusllm
./scripts/quickstart.sh

# Start infrastructure
cd docker && docker-compose up -d postgres redis ipfs jaeger prometheus grafana

# Start orchestrator services
docker-compose up -d api-gateway worker-registry model-registry scheduler settlement

# Start worker (GPU)
docker-compose up -d worker-gpu

# Or CPU-only worker
docker-compose up -d worker-cpu

# Submit inference
curl -X POST http://localhost:8080/v1/inference \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3-8b-instruct", "prompt": "Hello!", "stream": true}'
```

### Kubernetes (Production)

```bash
helm repo add nexusllm https://charts.nexusllm.network
helm install nexusllm nexusllm/nexusllm \
  --set workerDaemonSet.enabled=true \
  --set global.logLevel=info
```

### Binary Release

```bash
# Build
cargo build --release --bin nexus-agent --features cuda

# Run
./target/release/nexus-agent --config config/worker.toml
```

---

## Configuration

### Worker Config (`config/worker.toml`)

```toml
orchestrator_url = "ws://orchestrator.nexusllm.network"
model_cache_dir = "./model_cache"
tee_enabled = true
heartbeat_interval_secs = 10
attestation_interval_secs = 300
max_concurrent_tasks = 1
log_level = "info"
metrics_port = 9090

[stake_info]
worker_address = "0x..."          # Your wallet address
self_stake = 10000000000000000000000  # 10,000 NEXUS (18 decimals)

[metadata]
operator = "my-operator-name"
location = "us-west-2"
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `NEXUS_CONFIG` | Config file path | `./config.toml` |
| `NEXUS_ORCHESTRATOR_URL` | Orchestrator WebSocket | `wss://orchestrator.nexusllm.network` |
| `NEXUS_MODEL_CACHE_DIR` | Model cache directory | `./model_cache` |
| `NEXUS_TEE_ENABLED` | Enable TEE execution | `true` |
| `NEXUS_LOG_LEVEL` | Log level | `info` |
| `NEXUS_METRICS_PORT` | Prometheus metrics port | `9090` |

### Orchestrator Services Config

Each service (`api-gateway`, `worker-registry`, `model-registry`, `scheduler`, `settlement`) uses environment variables:

```bash
# Common
RUST_LOG=info
NEXUS_DB_URL=postgres://nexus:nexus@postgres:5432/nexus
NEXUS_REDIS_URL=redis://redis:6379
NEXUS_OTEL_ENDPOINT=http://jaeger:4317

# Service-specific
NEXUS_WORKER_REGISTRY_URL=http://worker-registry:8081
NEXUS_MODEL_REGISTRY_URL=http://model-registry:8082
NEXUS_SCHEDULER_URL=http://scheduler:8083
NEXUS_SETTLEMENT_URL=http://settlement:8084
NEXUS_IPFS_API=/ip4/ipfs/tcp/5001
NEXUS_CDN_BASE_URL=https://cdn.nexusllm.network
NEXUS_ETHEREUM_RPC=https://arb1.arbitrum.io/rpc
NEXUS_CONTRACT_ADDRESS=0x...
```

---

## API Reference

### Base URL
```
Production:  https://api.nexusllm.network/v1
Local:       http://localhost:8080/v1
```

### Authentication
- JWT (RS256) in `Authorization: Bearer <token>`
- API Keys for service-to-service
- Rate limiting per tier

### Endpoints

#### Inference

```http
POST /v1/inference
Content-Type: application/json
Authorization: Bearer <token>

{
  "model": "llama-3-8b-instruct",
  "version": "v1.2.0",           // optional, defaults to latest
  "prompt": "Hello, world!",
  "stream": true,
  "config": {
    "max_tokens": 512,
    "temperature": 0.7,
    "top_p": 0.9,
    "repetition_penalty": 1.1,
    "stop_token_ids": [128001, 128009],
    "speculative": {
      "enabled": true,
      "draft_model_id": "llama-3-8b-draft",
      "num_draft_tokens": 4
    }
  },
  "priority": "NORMAL"  // LOW, NORMAL, HIGH, CRITICAL
}
```

**Response** (streaming SSE):
```
data: {"token_id": 1234, "text": "Hello", "logprob": -0.12, "is_final": false}
data: {"token_id": 5678, "text": ", world", "logprob": -0.05, "is_final": true}
data: [DONE]
```

#### Model Discovery

```http
GET /v1/models
```

```json
{
  "models": [
    {
      "model_id": "llama-3-8b-instruct",
      "latest_version": "v1.2.0",
      "architecture": "llama",
      "total_params": 8000000000,
      "available_quants": ["FP16", "INT4", "GGUF_Q4_K_M"],
      "estimated_cost_per_1k_tokens": 0.0001,
      "is_active": true
    }
  ],
  "total": 42
}
```

#### Task Status

```http
GET /v1/tasks/{task_id}
```

```json
{
  "request_id": "req_abc123",
  "task_id": "task_xyz789",
  "status": "COMPLETED",
  "chunks": [
    {"token_id": 1234, "text": "Hello", "logprob": -0.12, "is_final": false}
  ],
  "stats": {
    "prompt_tokens": 10,
    "completion_tokens": 50,
    "ttft_ms": 145.2,
    "tpot_ms": 32.1,
    "total_time_ms": 1750.5
  }
}
```

#### WebSocket (Real-time)

```javascript
const ws = new WebSocket('wss://api.nexusllm.network/v1/realtime');
ws.send(JSON.stringify({ model: 'llama-3-8b-instruct', prompt: 'Hi' }));
ws.onmessage = (e) => console.log(JSON.parse(e.data));
```

---

## Worker Node Operations

### Starting a Worker

```bash
# With config file
nexus-agent --config /path/to/worker.toml

# With environment variables
NEXUS_ORCHESTRATOR_URL=ws://localhost:8080 \
NEXUS_MODEL_CACHE_DIR=/mnt/models \
nexus-agent
```

### Docker (GPU)

```bash
docker run -d --gpus all \
  -e NEXUS_ORCHESTRATOR_URL=ws://orchestrator:8080 \
  -v /mnt/models:/home/nexus/model_cache \
  -v ./config/worker.toml:/home/nexus/config.toml:ro \
  ghcr.io/nexusllm/nexus-agent:latest
```

### Monitoring Worker Health

```bash
# Metrics endpoint
curl http://localhost:9090/metrics

# Key metrics to watch
nexus_worker_online{peer_id="..."} 1
nexus_vram_usage_percent{peer_id="..."} 67.5
nexus_task_duration_seconds_bucket{le="1.0"} 1247
nexus_attestation_failure_total{peer_id="..."} 0
```

### Upgrading

```bash
# Auto-update (signed OTA)
nexus-agent --auto-update

# Manual
systemctl stop nexus-agent
cp new_binary /usr/local/bin/nexus-agent
systemctl start nexus-agent
```

### Troubleshooting Worker Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| Registration rejected | Stake too low / TEE failed | Increase stake, check TEE |
| No tasks assigned | Capabilities don't match | Verify GPU/TEE detection |
| High task failure | OOM / driver crash | Reduce batch, update drivers |
| Attestation failing | TEE not available / config | Check `/dev/sgx_enclave`, BIOS |
| P2P connection failed | NAT / firewall | Enable UPnP, configure TURN |

---

## Orchestrator Operations

### Service Health Checks

```bash
# All services
curl http://api-gateway:8080/health
curl http://worker-registry:8081/health
curl http://model-registry:8082/health
curl http://scheduler:8083/health
curl http://settlement:8084/health
```

### Scaling

```bash
# Horizontal scale API Gateway
kubectl scale deployment api-gateway --replicas=5

# Add coordinator nodes
helm upgrade nexusllm --set coordinator.replicas=3
```

### Model Onboarding

```bash
# Upload model to IPFS
ipfs add -r ./models/llama-3-70b/
# Returns CID: QmXYZ...

# Register in Model Registry
curl -X POST http://model-registry:8082/v1/models \
  -H "Authorization: Bearer <admin-token>" \
  -d '{
    "model_id": "llama-3-70b-instruct",
    "version": "v1.0.0",
    "architecture": "llama",
    "shards": [...],
    "ipfs_cid": "QmXYZ..."
  }'
```

### Managing Workers

```bash
# List active workers
curl http://worker-registry:8081/v1/workers

# Jail misbehaving worker
curl -X POST http://worker-registry:8081/v1/workers/{peer_id}/jail \
  -H "Authorization: Bearer <admin-token>"

# Adjust slashing parameters
curl -X PATCH http://settlement:8084/v1/params \
  -H "Authorization: Bearer <admin-token>" \
  -d '{"slash_incorrect_output_bps": 2500}'
```

---

## Model Sharding & Pipeline

### Shard Manifest Format

```json
{
  "model_id": "llama-3-70b-instruct",
  "version": "v1.2.0",
  "architecture": "llama",
  "total_params": 70000000000,
  "shards": [
    {
      "shard_id": "layers_0_15",
      "layers": [0, 15],
      "hash": "sha256:abc123...",
      "size_bytes": 14000000000,
      "quantization": "FP16",
      "min_vram_gb": 16,
      "supported_backends": ["cuda", "metal", "rocm"]
    }
  ],
  "pipeline_config": {
    "num_stages": 4,
    "stage_layers": [18, 18, 18, 16],
    "kv_cache_per_token_kb": 2.8
  }
}
```

### Parallelism Strategies

#### Pipeline Parallelism (Default)

```
Input → [L0-L15] → [L16-L31] → [L32-L47] → [L48-L63] → Output
         Worker A     Worker B     Worker C     Worker D
              │            │            │            │
              └────────────┴────────────┴────────────┘
                              │
                        KV Cache Transfer
                        (WebRTC DataChannel)
```

**Micro-batching** overlaps compute + communication:
```
Time →
Worker A: [Fwd 1] [Fwd 2] [Fwd 3] [Fwd 4] ...
Worker B:       [Fwd 1] [Fwd 2] [Fwd 3] ...
Worker C:             [Fwd 1] [Fwd 2] ...
Worker D:                   [Fwd 1] ...
```

#### Tensor Parallelism

```
Attention Heads Split (4 workers):
  QKV Proj → [H0-H7] [H8-H15] [H16-H23] [H24-H31]
                │         │         │         │
             All-Reduce (Ring) over WebRTC
                │         │         │         │
             Output Projection (merged)
```

#### Expert Parallelism (MoE)

```
Router → Expert 0 (Worker A) ──┐
      → Expert 1 (Worker B) ───┤
      → Expert 2 (Worker C) ───┼→ Merge (Coordinator)
      → Expert 3 (Worker D) ───┘
      
Only 2 experts active per token (top-2 routing)
```

### KV Cache Management

**Challenge**: 2.8 KB/token for 70B model

**Solutions**:
1. **Coordinator Aggregation**: Dedicated node merges cross-node KV
2. **Compression**: INT8 (2x), FP8 (2x), KIVI (8x), H2O/SnapKV (token dropping)
3. **Offloading**: CPU RAM ↔ VRAM async swap with prefetch
4. **P2P Distribution**: Shard KV across workers

### Speculative Decoding

```
Draft Model (Edge/CPU) → 4-8 tokens → Target Model (GPU Pipeline)
                              ↓
                        Verify all in parallel
                              ↓
                        Accept prefix until mismatch
                              ↓
                        Generate correction if needed

Speedup: 2-3x for typical chat workloads
```

### Dynamic Resharding

Triggers: worker join/leave, performance degradation, new model version, load imbalance.

Protocol:
1. Scheduler detects → computes new plan
2. Broadcasts `ReshardCommand` via GossipSub
3. Workers checkpoint KV + state
4. Coordinated swap (2-phase commit)
5. Resume from checkpoint

---

## Security & TEE

### Threat Model & Mitigations

| Threat | Mitigation |
|--------|------------|
| Malicious worker returns wrong output | Redundant execution (2x) + challenge sampling |
| Worker steals model weights | TEE encryption, weights never in host memory plaintext |
| Worker leaks user prompts | Input encryption via TEE-derived keys |
| Sybil attack (fake workers) | Hardware-bound identity (TPM) + stake requirement |
| Coordinator censorship | Multi-coordinator consensus, slashing for censorship |
| Network partition | Local queue + eventual consistency |

### TEE Attestation Flow

```
1. Worker generates ephemeral keypair inside TEE
2. TEE produces quote (hardware-signed measurement)
3. Orchestrator verifies quote against TCB info
4. Session key derived via ECDH (TEE key + orchestrator key)
5. All task I/O encrypted with session key
6. ZK proof generated over execution trace inside TEE
```

### Zero-Knowledge Proof of Compute

- **Circuit**: RISC Zero / SP1 / Halo2
- **Inputs**: Manifest hash, input/output tensor hashes, execution trace, TEE quote
- **Constraints**: Valid trace, matching outputs, valid attestation
- **Proof Size**: ~100-500 KB (Groth16) / ~50-100 KB (PLONK)
- **Verification**: ~5-10ms on-chain / ~1ms off-chain

### Network Security

- **Transport**: Noise XX (libp2p) + DTLS 1.3 (WebRTC)
- **Auth**: Mutual Ed25519 (hardware-bound), forward secrecy (X25519)
- **Post-Quantum**: Hybrid KEM (X25519 + Kyber768) — optional

---

## Economics & Settlement

### Token: NEXUS

- **Standard**: ERC-20 (Arbitrum) / SPL (Solana)
- **Decimals**: 18
- **Min Stake**: 10,000 NEXUS (~$500 at launch)
- **Target APR**: 15-25% for honest operators
- **Inflation**: 5% annually → 2%
- **Protocol Fee**: 2% of inference spend → treasury

### Reward Formula

```
Reward = base_reward × quality_multiplier × capability_multiplier × uptime_multiplier

base_reward = token_per_flop × task_flops × (1 + priority_fee)
quality_multiplier = 0.5 + 0.5 × (correct_samples / total_samples)
capability_multiplier = log2(vram_gb) / log2(max_vram) × backend_efficiency
uptime_multiplier = min(1.0, uptime_hours / 720)  // caps at 30 days
```

### Slashing Conditions

| Condition | Slash % | Detection |
|-----------|---------|-----------|
| Failed attestation | 10% | TEE quote verification |
| Incorrect output (challenge) | 25% | Redundant execution |
| Incorrect output (ZK proof) | 50% | On-chain verification |
| Extended downtime (>1hr) | 5%/hr | Heartbeat missing |
| Censorship | 15% | Multi-coordinator audit |

### Delegation (Liquid Staking)

```bash
# Stake directly
nexusctl stake --worker 0xWorker... --amount 10000

# Delegate to operator
nexusctl delegate --operator 0xOperator... --amount 5000
# Receives stNEXUS (liquid staking token)
```

---

## Monitoring & Observability

### Stack

```
Workers → OTel Collector (sidecar) → Kafka/NATS
                ↓
    ┌───────────┼───────────┐
    ▼           ▼           ▼
Tempo      Prometheus     Loki
(Traces)   (Metrics)      (Logs)
    │           │           │
    └───────────┼───────────┘
                ▼
          Grafana Dashboards
          Alertmanager → PagerDuty/Slack
```

### Key Dashboards

| Dashboard | Metrics |
|-----------|---------|
| Worker Health | `nexus_worker_online`, VRAM/CPU usage, temperature |
| Inference Latency | `nexus_ttft_seconds`, `nexus_tpot_seconds` by model |
| KV Cache Transfer | `nexus_kv_transfer_bytes`, throughput, latency |
| Attestation | Success rate, quote verification time |
| Economics | Rewards distributed, slashing events, APR |
| Network Topology | Peer connections, relay usage, NAT types |

### Alerting Rules

```yaml
# Critical alerts
- alert: WorkerOffline
  expr: nexus_worker_online == 0
  for: 2m
  
- alert: AttestationFailureRate
  expr: rate(nexus_attestation_failure_total[5m]) > 0.1
  for: 1m
  
- alert: HighSlashingRate
  expr: rate(nexus_slashing_events_total[1h]) > 0.01
  for: 5m
  
- alert: InferenceLatencyP99
  expr: histogram_quantile(0.99, rate(nexus_task_duration_seconds_bucket[5m])) > 30
  for: 2m
```

---

## Development Guide

### Project Structure

```
nexusllm/
├── crates/                      # Rust workspace
│   ├── nexus-agent/             # Worker daemon
│   ├── nexus-common/            # Shared types & utilities
│   ├── nexus-executor/          # Shard executor + backends
│   ├── nexus-tee/               # TEE abstraction
│   ├── nexus-p2p/               # libp2p + WebRTC networking
│   ├── nexus-scheduler/         # DAG scheduler
│   ├── nexus-registry/          # Model + Worker registry
│   ├── nexus-settlement/        # Settlement engine
│   └── nexus-proto/             # Protobuf definitions
├── orchestrator/                # Cloud control plane
│   ├── api-gateway/             # REST + WebSocket
│   ├── scheduler-service/       # gRPC scheduling
│   ├── registry-service/        # Registries
│   ├── settlement-service/      # Settlement processor
│   └── observability/           # OTel, Grafana configs
├── contracts/                   # Smart contracts (Solidity/Rust)
├── sdk/                         # Client SDKs (Python, TS, Go, Rust)
├── cli/                         # nexusctl CLI
├── docker/                      # Dockerfiles, docker-compose
├── helm/                        # Kubernetes Helm charts
├── docs/                        # Documentation
├── scripts/                     # Utility scripts
├── benchmarks/                  # Performance tests
└── tests/                       # Integration, chaos, load tests
```

### Building

```bash
# Full workspace
cargo build --release --workspace

# Specific backend
cargo build --release --bin nexus-agent --features cuda
cargo build --release --bin nexus-agent --features metal
cargo build --release --bin nexus-agent --features llama-cpp

# Protobuf regeneration
cd crates/nexus-proto && cargo build
```

### Testing

```bash
# Unit tests
cargo test --workspace

# Integration tests
cargo test --test integration

# Chaos testing (requires cluster)
cargo test --test chaos -- --ignored

# Load testing
cargo test --test load -- --ignored

# Benchmarks
cargo bench --workspace
```

### Code Style

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace -- -D warnings

# Protobuf lint
buf lint
```

### Adding a New Backend

1. Create `crates/nexus-executor/new-backend/`
2. Implement `ShardExecutor` trait
3. Add feature flag in `nexus-executor/Cargo.toml`
4. Update `create_best_executor()` priority order
5. Add backend-specific dependencies (CUDA, Metal, etc.)
6. Write benchmarks in `benches/`

---

## Troubleshooting

### Common Issues

#### Worker Won't Register

```bash
# Check stake
nexusctl stake-info --worker 0xWorker...

# Verify TEE
ls /dev/sgx_enclave /dev/sev /dev/tee0

# Check orchestrator connectivity
curl -v ws://orchestrator:8080/health
```

#### Out of Memory

```bash
# Reduce batch size in config
max_concurrent_tasks = 1

# Enable KV offloading
kv_offload_enabled = true
kv_offload_threshold_gb = 8

# Use lower quantization
preferred_quantization = "INT4"
```

#### Slow Inference

```bash
# Check pipeline efficiency
grep "pipeline_efficiency" logs/

# Verify WebRTC data channel established
grep "WebRTC.*connected" logs/

# Check for tensor parallel opportunity
# Enable if >8 GPUs available
enable_tensor_parallel = true
```

#### P2P Connection Issues

```bash
# Check NAT type
nexusctl p2p-info --worker 0xWorker...

# Add TURN server
turn_servers = ["turn:turn.nexusllm.network:3478?transport=udp"]

# Force relay
force_relay = true
```

### Debug Commands

```bash
# Worker status
nexusctl status --worker 0xWorker...

# Task history
nexusctl tasks --worker 0xWorker... --limit 100

# P2P peers
nexusctl p2p-peers --worker 0xWorker...

# Attestation verify
nexusctl attestation-verify --quote <base64> --measurement <hex>

# ZK proof verify
nexusctl zk-verify --proof <base64> --inputs <json>
```

---

## License

This project is licensed under the **NexusLLM License v1.0** — see [LICENSE](LICENSE) for details.

**Summary**:
- ✅ **Free for non-commercial use** (research, education, personal projects)
- 💰 **Commercial use requires 20% royalty** on net revenue attributable to the software
- 📋 Attribution required in all distributions

For commercial licensing inquiries: **licensing@nexusllm.network**

---

## Appendix: File Inventory

### Root Files
| File | Purpose |
|------|---------|
| `README.md` | Project overview, quickstart, architecture |
| `LICENSE` | NexusLLM License v1.0 (20% commercial royalty) |
| `NOTICE` | Attribution & third-party license inventory |
| `Cargo.toml` | Workspace root manifest |
| `distributed-llm-architecture.html` | Interactive architecture diagram |
| `NEXUSLLM_DESIGN.md` | 27KB complete technical design |
| `proto/nexus.proto` | Protocol Buffers schema |
| `docker/docker-compose.yml` | Local dev stack |
| `docker/worker/Dockerfile` | GPU worker container |
| `helm/nexusllm/` | Kubernetes deployment |
| `scripts/quickstart.sh` | One-command setup |

### Rust Crates (10)
| Crate | Lines | Purpose |
|-------|-------|---------|
| `nexus-agent` | ~2,000 | Worker daemon |
| `nexus-common` | ~500 | Shared types, crypto, metrics |
| `nexus-executor` | ~3,000 | ShardExecutor trait + 5 backends |
| `nexus-tee` | ~4,000 | TEE abstraction (4 platforms) |
| `nexus-p2p` | ~500 | libp2p + WebRTC |
| `nexus-scheduler` | ~400 | DAG planner |
| `nexus-registry` | ~500 | Model/Worker registry clients |
| `nexus-settlement` | ~400 | On/off-chain settlement |
| `nexus-proto` | ~100 | Protobuf build.rs |
| `orchestrator/*` | ~500 each | 5 cloud services |

### Documentation Files
| File | Description |
|------|-------------|
| `docs/ARCHITECTURE_ROADMAP.md` | Phased implementation plan |
| `docs/TECHNICAL_WHITEPAPER.md` | Deep-dive technical paper |
| `docs/THREAT_MODEL.md` | Security threat analysis |
| `docs/SECURITY_OPERATIONS.md` | SecOps runbooks |
| `docs/API_SPECIFICATION.md` | Complete OpenAPI spec |
| `docs/adr/*.md` | Architecture Decision Records |

---

*Built with ❤️ by the NexusLLM community — Democratizing AI compute, one home desktop at a time.*