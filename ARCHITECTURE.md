# NexusLLM Architecture Deep-Dive

## System Overview

NexusLLM is a **distributed inference platform** that enables anyone to run any LLM by pooling compute from home desktops worldwide. The architecture separates the **control plane** (orchestration, scheduling, settlement) from the **data plane** (P2P model execution, KV cache transfer).

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              USER LAYER                                       │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐           │
│  │  CLI    │  │ Python  │  │  REST   │  │WebSocket│  │  Web UI │           │
│  │  SDK    │  │  SDK    │  │  API    │  │  API    │  │         │           │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘           │
└───────┼─────────────┼─────────────┼─────────────┼─────────────┼──────────────┘
        │             │             │             │             │
        ▼             ▼             ▼             ▼             ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                        ORCHESTRATOR (Control Plane)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ API Gateway  │  │  Scheduler   │  │ Model Registry│  │ Worker Registry │  │
│  │  (Stateless) │  │  (Stateless) │  │  (IPFS+CDN)  │  │  (Redis+P2P)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬─────────┘  │
│         │                 │                 │                 │            │
│         └─────────────────┼─────────────────┼─────────────────┘            │
│                           ▼                 ▼                              │
│                  ┌─────────────────────────────────┐                       │
│                  │      Message Bus (NATS)         │                       │
│                  │  Tasks  │  Heartbeats  │ Events │                       │
│                  └─────────────────────────────────┘                       │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                    ┌─────────────────┼─────────────────┐
                    ▼                 ▼                 ▼
            ┌───────────────┐ ┌───────────────┐ ┌───────────────┐
            │ Coordinator 1 │ │ Coordinator 2 │ │ Coordinator N │
            │  (Backbone)   │ │  (Backbone)   │ │  (Backbone)   │
            └───────┬───────┘ └───────┬───────┘ └───────┬───────┘
                    │                 │                 │
        ┌───────────┼───────────┐     │     ┌───────────┼───────────┐
        ▼           ▼           ▼     ▼     ▼           ▼           ▼
    ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
    │Worker 1│ │Worker 2│ │Worker 3│ │Worker 4│ │Worker 5│ │Worker N│
    │RTX 4090│ │RTX 3080│ │M3 Max  │ │CPU Only│ │Mobile  │ │ ...    │
    └────────┘ └────────┘ └────────┘ └────────┘ └────────┘ └────────┘
```

---

## Core Components

### 1. Nexus Agent (Worker Daemon)

**Location**: `crates/nexus-agent/`

**Responsibilities**:
- Hardware identity derivation (TPM-bound Ed25519 key)
- Capability discovery (GPU/CPU, VRAM, instruction sets, TEE)
- Task lifecycle: pull → verify manifest → execute → prove → submit
- Model cache management (4-tier: VRAM → Unified RAM → NVMe → P2P)
- Heartbeat emission (10s interval via GossipSub)
- TEE attestation refresh (5 min interval)

**Architecture**:
```
┌─────────────────────────────────────────────────────────────┐
│                    Nexus Agent                               │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ P2P Node    │  │TEE Manager  │  │Shard Executor│         │
│  │(libp2p+WebRTC)│  │(SGX/SEV/SE) │  │(CUDA/Metal/  │          │
│  │             │  │             │  │ CPU/WASM)   │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                │                  │
│         ▼                ▼                ▼                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Task Runner                             │    │
│  │  pull → verify → execute (in TEE) → prove → submit  │    │
│  └─────────────────────────────────────────────────────┘    │
│         │                │                │                  │
│         ▼                ▼                ▼                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Heartbeat   │  │ Attestation │  │ Model Cache │          │
│  │ Manager     │  │ Manager     │  │ (4-tier)    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 2. Shard Executor (Unified Inference Interface)

**Trait**:
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

**Backend Priority** (auto-selected):
1. **CUDA** — NVIDIA GPU (FlashAttention-2, FP8, tensor parallel)
2. **Metal** — Apple Silicon (MLX, unified memory)
3. **ROCm** — AMD GPU (FlashAttention, HIP)
4. **llama.cpp** — CPU (GGUF 4-bit, AVX2/AMX/NEON)
5. **WASM** — Browser/Edge (WebGPU, WebLLM)

### 3. TEE Abstraction Layer

**Platform Support**:
| TEE | Platform | Attestation | Sealing | Key Derivation |
|-----|----------|-------------|---------|----------------|
| SGX | Intel x86 | DCAP Quote | EPC Sealing | ECDH in EPC |
| SEV-SNP | AMD x86 | SNP Report | Memory Encryption | VMPCK |
| TrustZone | ARM | TEE Gratis | Secure Storage | TEE Internal |
| Secure Enclave | Apple | App Attest | Keychain | P256 in SE |
| Software | Any | None | AES-GCM | HKDF |

**Attestation Flow**:
```
1. Worker generates ephemeral keypair INSIDE TEE
2. TEE produces quote (hardware-signed measurement)
3. Orchestrator verifies quote against known TCB
4. Session key derived via ECDH (TEE key + orchestrator key)
5. All task I/O encrypted with session key
6. ZK proof generated over execution trace INSIDE TEE
```

### 4. P2P Networking (Data Plane)

**Stack**:
- **Transport**: QUIC (primary), WebSocket, WebRTC, TCP
- **NAT Traversal**: ICE + STUN + TURN (incentivized relays)
- **Discovery**: mDNS (LAN), Kademlia DHT (WAN), bootstrap nodes
- **PubSub**: GossipSub for heartbeats, tasks, reshards
- **Data Channels**: WebRTC DataChannels (ordered, reliable) for KV cache

**Mesh Topology**:
```
Coordinators (staked, high-bandwidth) ←→ Workers (leaves, 3-5 connections)
                                             ↳ Edge/Mobile (relay-assisted)
```

**KV Cache Transfer**:
```
Worker A (Stage 1)          Worker B (Stage 2)
    │                           │
    ├─ Forward Pass 1 ───────► │
    │                           │
    ├─ Forward Pass 2 ───────► │  (micro-batching overlap)
    │                           │
    │          KV Cache ──────►│  (WebRTC DataChannel, compressed)
    │                           │
```

---

## Task Scheduling & Pipeline

### Scheduler Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Task Scheduler                            │
├─────────────────────────────────────────────────────────────┤
│  1. Request → Model Registry → Shard Manifest               │
│  2. Filter Workers by capability (VRAM, backend, TEE)       │
│  3. Choose Parallelism Strategy:                            │
│     ├─ Pipeline: Sequential layer groups                   │
│     ├─ Tensor: Intra-layer split (attention heads, MLP)    │
│     ├─ Expert: MoE routing to specialized workers          │
│     └─ Speculative: Draft on edge → verify on GPU          │
│  4. Assign Shards (bin packing + affinity)                  │
│  5. Build DAG with micro-batch overlap                     │
│  5. Estimate latency + cost                                 │
└─────────────────────────────────────────────────────────────┘
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

Micro-batching overlap:
Time →
Worker A: [Fwd 1] [Fwd 2] [Fwd 3] [Fwd 4] ...
Worker B:       [Fwd 1] [Fwd 2] [Fwd 3] ...
Worker C:             [Fwd 1] [Fwd 2] ...
Worker D:                   [Fwd 1] ...
```

#### Tensor Parallelism
```
Attention Heads Split (4 workers):
  QKV Projection → [H0-H7] [H8-H15] [H16-H23] [H24-H31]
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

### Dynamic Resharding

**Triggers**: Worker join/leave, thermal throttling, OOM, new model version, load imbalance.

**Protocol**:
1. Scheduler detects → computes new plan
2. Broadcasts `ReshardCommand` via GossipSub
3. Workers checkpoint KV + state
4. Coordinated swap (2-phase commit)
5. Resume from checkpoint (<5s)

---

## Model Registry & Storage

### Content-Addressable Storage

```
Model Manifest (JSON + Merkle)
    │
    ├─ Shard 1: layers_0_15  → IPFS CID + CDN mirrors
    ├─ Shard 2: layers_16_31 → IPFS CID + CDN mirrors
    └─ Shard 3: layers_32_47 → IPFS CID + CDN mirrors
```

**Manifest Format**:
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

### Delta Updates

- Binary diffs (bsdiff) between versions
- Verify via Merkle proofs
- Typical delta: 1-5% of full model size

---

## Settlement & Economics

### On-Chain (Arbitrum/Solana)

**Contracts**:
- `Staking` — stake/unstake, delegation, liquid stNEXUS
- `Rewards` — epoch-based distribution, quality multipliers
- `Slashing` — failed attestation, incorrect output, downtime
- `Governance` — parameter updates, DAO

### Off-Chain (Optimistic)

- Micro-rewards per task (orchestrator-signed)
- Batch settlement every 1000 tasks or 1 hour
- Dispute window: 7 days (challenge via ZK proof)

### Reward Formula

```
Reward = base_reward × quality_multiplier × capability_multiplier × uptime_multiplier

base_reward = token_per_flop × task_flops × (1 + priority_fee)
quality_multiplier = 0.5 + 0.5 × (correct_samples / total_samples)
capability_multiplier = log2(vram_gb) / log2(max_vram) × backend_efficiency
uptime_multiplier = min(1.0, uptime_hours / 720)
```

### Slashing Conditions

| Condition | Slash % | Detection |
|-----------|---------|-----------|
| Failed attestation | 10% | TEE quote verification |
| Incorrect output (challenge) | 25% | Redundant execution |
| Incorrect output (ZK proof) | 50% | On-chain verification |
| Extended downtime (>1hr) | 5%/hr | Heartbeat missing |
| Censorship | 15% | Multi-coordinator audit |

---

## Observability Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    OBSERVABILITY PIPELINE                    │
├─────────────────────────────────────────────────────────────┤
│  Workers → OTel Collector (sidecar) → Kafka/NATS            │
│       ↓                                                     │
│  Tempo (traces) ──┬── Grafana (dashboards)                  │
│  Prometheus (metrics) ──┬── Alertmanager (pagerduty)        │
│  Loki (logs) ──────┘                                      │
└─────────────────────────────────────────────────────────────┘
```

### Key Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `nexus_worker_online` | gauge | Worker online status |
| `nexus_task_duration_seconds` | histogram | Task execution time |
| `nexus_ttft_seconds` | histogram | Time to first token |
| `nexus_tpot_seconds` | histogram | Time per output token |
| `nexus_kv_transfer_bytes` | histogram | Inter-worker KV transfer |
| `nexus_attestation_failure_total` | counter | TEE failures |
| `nexus_reward_tokens_distributed` | counter | Economics |
| `nexus_slashing_events_total` | counter | Security |

---

## Security Architecture

### Threat Model

| Threat | Likelihood | Impact | Mitigation |
|--------|------------|--------|------------|
| Malicious output | High | High | Redundant execution + challenge sampling |
| Weight theft | Medium | Critical | TEE encryption, never in host plaintext |
| Prompt leakage | Medium | High | Input encryption via TEE-derived keys |
| Sybil attack | High | High | TPM-bound identity + stake requirement |
| Coordinator censorship | Low | High | Multi-coordinator consensus + slashing |
| Network partition | Medium | Medium | Local queue + eventual consistency |

### Zero-Knowledge Proof of Compute

- **Circuit**: RISC Zero / SP1 / Halo2
- **Inputs**: Manifest hash, input/output hashes, execution trace, TEE quote
- **Constraints**: Valid trace, matching outputs, valid attestation
- **Proof Size**: ~100-500 KB (Groth16) / ~50-100 KB (PLONK)
- **Verification**: ~5-10ms on-chain / ~1ms off-chain

### Network Security

- **Transport**: Noise XX (libp2p) + DTLS 1.3 (WebRTC)
- **Auth**: Mutual Ed25519 (hardware-bound), forward secrecy (X25519)
- **Post-Quantum**: Hybrid KEM (X25519 + Kyber768) — optional

---

## Deployment Architectures

### Single-Region (Development)

```
┌─────────────────────────────────────────────┐
│            Docker Compose                    │
├─────────────────────────────────────────────┤
│  API Gateway  │ Worker Registry │ Scheduler │
│  Model Reg    │ Settlement      │           │
│  PostgreSQL   │ Redis           │ IPFS      │
│  Jaeger       │ Prometheus      │ Grafana   │
└─────────────────────────────────────────────┘
```

### Multi-Region Production (Kubernetes)

```
┌─────────────────────────────────────────────────────────────┐
│                    Global Load Balancer                      │
└─────────────────────────────────────────────────────────────┘
                    │              │              │
        ┌───────────┴───────────┐  ▼  ┌───────────┴───────────┐
        ▼                       ▼     ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│  us-west-2    │       │  eu-west-1    │       │  ap-southeast-1│
│  (Primary)    │       │  (Secondary)  │       │  (Edge)       │
├───────────────┤       ├───────────────┤       ├───────────────┤
│ API Gateway   │       │ API Gateway   │       │ API Gateway   │
│ Scheduler     │◄─────►│ Scheduler     │◄─────►│ Scheduler     │
│ Worker Reg    │  Sync │ Worker Reg    │  Sync │ Worker Reg    │
│ Model Reg     │       │ Model Reg     │       │ Model Reg     │
│ Settlement    │       │ Settlement    │       │ Settlement    │
│ PostgreSQL    │       │ PostgreSQL    │       │ PostgreSQL    │
│ (Primary)     │       │ (Replica)     │       │ (Replica)     │
└───────────────┘       └───────────────┘       └───────────────┘
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                ▼
                    ┌─────────────────────┐
                    │   Global IPFS       │
                    │   Cluster           │
                    └─────────────────────┘
```

---

## Scaling Considerations

### Horizontal Scaling

| Component | Scaling Strategy |
|-----------|------------------|
| API Gateway | Stateless, HPA by CPU/requests |
| Scheduler | Single-leader (Raft), read replicas |
| Worker Registry | Redis Cluster + P2P gossip |
| Model Registry | IPFS cluster + CDN |
| Settlement | Single-leader, async processing |
| Workers | P2P mesh, auto-discovery |

### Capacity Planning

| Workers | Models Supported | Peak Throughput |
|---------|------------------|-----------------|
| 100 | 7B-13B | 500 req/s |
| 1,000 | 7B-70B | 5,000 req/s |
| 10,000 | 7B-400B+ | 50,000 req/s |
| 100,000 | All | 500,000 req/s |

---

## Future Architecture Evolution

### Phase 1: Foundation ✅
- Basic orchestrator + worker agent
- Single-model pipeline parallelism
- P2P networking

### Phase 2: Heterogeneity & Security ✅
- Multi-architecture backends
- TEE integration
- ZK proofs + settlement

### Phase 3: Advanced Sharding 🔄
- Tensor + expert parallelism
- Dynamic resharding
- Speculative decoding

### Phase 4: Network Effects 📋
- Mobile/WASM workers
- Marketplace + DAO
- Multi-region federation

### Phase 5: Research 🔬
- Communication-optimal 3D parallelism
- FHE for sensitive prompts
- Learned model routing
- Hardware co-design

---

## Appendix: Key Data Structures

### Protobuf Schema (Key Messages)

```protobuf
message InferenceRequest {
  string request_id = 1;
  string model_id = 2;
  string version = 3;
  oneof content { string text = 4; repeated ChatMessage messages = 5; }
  GenerationConfig config = 6;
  Priority priority = 7;
}

message TaskAssignment {
  string task_id = 1;
  repeated ShardAssignment shards = 2;
  ExecutionPlan plan = 3;
  bytes session_key = 4;  // Encrypted for worker (TEE-derived)
}

message ShardAssignment {
  string shard_id = 1;
  string worker_peer_id = 2;
  uint32 stage_index = 3;
  KVCacheConfig kv_config = 4;
}

message ExecutionResult {
  string task_id = 1;
  bool success = 2;
  bytes output_payload = 3;  // Encrypted
  bytes zk_proof = 4;
  bytes tee_quote = 5;
}
```

---

*This architecture document is a living document. See [GitHub](https://github.com/nexusllm/nexusllm) for the latest updates.*