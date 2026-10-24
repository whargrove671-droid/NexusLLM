# NexusLLM: Distributed LLM Compute Platform
## Complete Technical Design Document

---

## 1. Executive Summary

**NexusLLM** is a decentralized platform that aggregates heterogeneous compute resources (home desktops, laptops, mobile devices) into a unified inference cluster capable of running large language models (7B–400B+ parameters) with performance approaching dedicated data-center deployments.

### Key Differentiators
- **Heterogeneous-first design**: Native support for NVIDIA CUDA, AMD ROCm, Apple Metal/MLX, Intel AMX, ARM NEON, WASM
- **TEE-secured execution**: Hardware attestation (SGX/SEV/TrustZone/Secure Enclave) for proof-of-compute and data privacy
- **P2P data plane**: libp2p + WebRTC mesh eliminates central bandwidth bottlenecks for KV cache transfer
- **Dynamic sharding**: Pipeline + tensor + expert parallelism with runtime rebalancing
- **Crypto-economic incentives**: ZK-verified proof-of-compute, stake-weighted sybil resistance, liquid delegation

---

## 2. Client-Side Components (Worker Node)

### 2.1 Nexus Agent (Daemon)
**Language**: Rust (compiled to native + WASM for browser/edge)
**Responsibilities**:
- Device capability discovery (VRAM, compute units, instruction sets, TEE availability)
- Heartbeat + metrics emission (Prometheus exposition format)
- Task lifecycle: pull → verify manifest → execute → prove → submit
- Auto-update via signed OTA manifests
- Resource isolation: cgroups (Linux), Job Objects (Windows), launchd (macOS)

```rust
// Core agent loop (simplified)
async fn run_agent(config: AgentConfig) -> Result<()> {
    let identity = HardwareIdentity::derive()?; // TPM/HSM-bound
    let registry = WorkerRegistryClient::new(config.orchestrator_url);
    registry.register(identity, capabilities).await?;

    loop {
        let task = registry.poll_task(identity).await?;
        let manifest = ModelRegistry::fetch_shard_manifest(task.model_id, task.shard_id).await?;
        
        // Verify shard integrity (Merkle proof)
        manifest.verify()?;
        
        // Execute in TEE if available
        let result = if config.tee_enabled {
            tee_execute(&manifest, task.input).await?
        } else {
            native_execute(&manifest, task.input).await?
        };
        
        // Generate ZK proof of execution
        let proof = zk_prove_execution(&manifest, task.input, &result).await?;
        
        registry.submit_result(task.id, result, proof).await?;
    }
}
```

### 2.2 Shard Executor (Inference Engine)
**Pluggable backends per hardware target**:

| Hardware | Backend | Quantization | Max Model |
|----------|---------|--------------|-----------|
| NVIDIA GPU | CUDA + cuBLASLt + FlashAttention-2 | FP16/BF16, INT8, FP8, GPTQ, AWQ | 70B+ (multi-GPU) |
| AMD GPU | ROCm + hipBLASLt + FlashAttention | FP16/BF16, INT8 | 70B+ |
| Apple Silicon | Metal + MLX | FP16, INT4, INT8 | 70B+ (unified mem) |
| Intel CPU | oneDNN + AMX + llama.cpp | GGUF 4-bit, Q8_0 | 30B |
| ARM CPU | llama.cpp + NEON | GGUF 4-bit | 13B |
| WASM/Browser | webllm / wasmedge | INT4 | 7B |
| Mobile NPU | ExecuTorch / CoreML / MNN | INT4/INT8 | 7B |

**Unified API** (Executor trait):
```rust
trait ShardExecutor: Send + Sync {
    fn load_shard(&mut self, manifest: &ShardManifest) -> Result<()>;
    fn forward(&mut self, input: TensorBatch) -> Result<TensorBatch>;
    fn get_kv_cache(&self) -> KVCacheHandle;
    fn set_kv_cache(&mut self, cache: KVCacheHandle);
    fn memory_usage(&self) -> MemoryStats;
}
```

### 2.3 Model Cache (Tiered Storage)
```
┌─────────────────────────────────────────────────────────────┐
│                    MODEL CACHE HIERARCHY                     │
├──────────────┬──────────────┬──────────────┬────────────────┤
│   TIER 0     │   TIER 1     │   TIER 2     │    TIER 3      │
│  (VRAM/HBM)  │ (Unified RAM)│   (NVMe)     │  (Network/P2P) │
├──────────────┼──────────────┼──────────────┼────────────────┤
│ Active shard │ Next 2-3     │ Full model   │ Remote peers   │
│ weights      │ shards       │ + quantized  │ (on-demand)    │
│ KV cache     │ Prefetched   │ variants     │                │
│ Latency:     │ Latency:     │ Latency:     │ Latency:       │
│ < 100μs      │ < 1ms        │ < 10ms       │ 10-500ms       │
└──────────────┴──────────────┴──────────────┴────────────────┘
```

**Prefetch policy**: Predictive based on pipeline position + attention sink analysis

### 2.4 TEE / Secure Enclave Integration

| Platform | TEE Technology | Attestation | Use Case |
|----------|----------------|-------------|----------|
| Intel x86 | SGX / TDX | Intel DCAP | Proof-of-compute, key sealing |
| AMD x86 | SEV-SNP | AMD KDS | Encrypted memory, attestation |
| Apple Silicon | Secure Enclave | Apple DeviceCheck | Key derivation, biometric binding |
| ARM | TrustZone / CCA | ARM CCA | Isolated execution, measured boot |
| NVIDIA GPU | Confidential Computing | NVIDIA CC | Encrypted GPU memory (H100+) |

**Attestation flow**:
```
1. Worker generates ephemeral keypair inside TEE
2. TEE produces quote (hardware-signed measurement)
3. Orchestrator verifies quote against TCB info
4. Session key derived via ECDH (TEE key + orchestrator key)
5. All task I/O encrypted with session key
6. ZK proof generated over execution trace inside TEE
```

### 2.5 P2P Networking Stack
```
libp2p Core
├── Transport: QUIC (preferred), WebSocket, WebRTC, TCP
├── NAT Traversal: ICE + STUN + TURN (incentivized relays)
├── Discovery: GossipSub (topic: /nexus/workers/{region}) + Kademlia DHT
├── Security: Noise XX + DTLS 1.3 (mutual auth)
├── PubSub: GossipSub for heartbeats, task announcements
└── Data Channels: WebRTC DataChannels (ordered, reliable) for KV cache
```

**Mesh topology**: Hierarchical
- **Backbone**: Coordinator nodes (high bandwidth, low latency, staked)
- **Leaf**: Worker nodes (connect to 3-5 coordinators)
- **Edge**: Mobile/browser nodes (relay-assisted via coordinators)

---

## 3. Server-Side Components (Orchestrator)

### 3.1 API Gateway
**Tech**: Axum (Rust) / Go + gRPC-Gateway
- **Endpoints**:
  - `POST /v1/inference` — Submit inference request (streaming SSE/WebSocket)
  - `GET /v1/models` — List available models + sharding configs
  - `GET /v1/tasks/{id}` — Poll task status
  - `WS /v1/realtime` — Bidirectional streaming for chat
- **Auth**: JWT (RS256) + API keys, rate limiting (token bucket per tier)
- **Observability**: OpenTelemetry middleware, request tracing

### 3.2 Task Scheduler (Core Intelligence)
**Algorithm**: Heterogeneous DAG scheduler with cost model

```python
# Scheduler cost model (simplified)
class Scheduler:
    def __init__(self, registry: WorkerRegistry, model_registry: ModelRegistry):
        self.registry = registry
        self.models = model_registry
    
    def plan(self, request: InferenceRequest) -> ExecutionPlan:
        model = self.models.get(request.model_id)
        workers = self.registry.get_eligible_workers(model)
        
        # 1. Determine optimal parallelism strategy
        strategy = self.choose_strategy(model, workers, request)
        
        # 2. Assign shards to workers (bin packing + affinity)
        assignments = self.assign_shards(model.shards, workers, strategy)
        
        # 3. Build pipeline DAG
        dag = self.build_dag(assignments, strategy)
        
        # 4. Estimate latency + cost
        latency = self.estimate_latency(dag, workers)
        cost = self.estimate_cost(dag, workers)
        
        return ExecutionPlan(dag=dag, estimated_latency=latency, estimated_cost=cost)
    
    def choose_strategy(self, model, workers, request):
        # Pipeline parallelism for sequential layers
        # Tensor parallelism for attention/MLP within layer
        # Expert parallelism for MoE
        # Speculative decoding for latency-critical
        pass
```

**Strategies**:
| Model Type | Primary Strategy | Fallback |
|------------|------------------|----------|
| Dense (LLaMA, Mistral) | Pipeline (layer groups) | Tensor (if >8 GPUs) |
| MoE (Mixtral, DeepSeek) | Expert + Pipeline | Tensor (shared experts) |
| Small (<7B) | Single-node + Speculative | Pipeline (if distributed) |
| Ultra-long context | Ring attention + KV offload | Pipeline + KV aggregation |

### 3.3 Model Registry
**Storage**: Content-addressable (IPFS/Filecoin + CDN cache)
**Manifest format** (JSON + Merkle tree):
```json
{
  "model_id": "llama-3-70b-instruct",
  "version": "v1.2.0",
  "architecture": "llama",
  "params": 70_000_000_000,
  "shards": [
    {
      "shard_id": "layers_0_15",
      "layers": [0, 15],
      "hash": "sha256:abc123...",
      "size_bytes": 14_000_000_000,
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

**Delta updates**: Binary diffs (bsdiff) between versions, verify via Merkle proofs

### 3.4 Worker Registry
**Data model**:
```rust
struct WorkerInfo {
    peer_id: PeerId,
    identity: HardwareIdentity,      // TPM-bound
    capabilities: DeviceCapabilities,
    status: WorkerStatus,            // Online/Offline/Busy/Maintenance
    reputation: ReputationScore,     // Quality, uptime, slashing history
    stake: StakeInfo,                // Self-stake + delegated
    last_heartbeat: Timestamp,
    network_info: NetworkInfo,       // NAT type, bandwidth, latency to coordinators
}
```

**Heartbeat protocol**: Every 10s via GossipSub, includes:
- Current load (VRAM%, compute%, network%)
- Active task ID (if any)
- Attestation freshness (TEE quote timestamp)

### 3.5 Settlement Engine
**On-chain** (Ethereum L2 / Solana / Custom rollup):
- **Staking**: `stake(worker_address, amount)` — minimum 10k NEXUS
- **Delegation**: `delegate(staker, operator, amount)` — liquid staking token (stNEXUS)
- **Rewards**: `distribute_rewards(epoch)` — pro-rata by compute contribution
- **Slashing**: `slash(worker, amount, reason)` — verified via ZK proof + attestation

**Off-chain** (optimistic):
- Micro-rewards per task (signed by orchestrator)
- Batch settlement every 1000 tasks or 1 hour
- Dispute window: 7 days (challenge via ZK proof verification)

### 3.6 Observability Stack
```
┌────────────────────────────────────────────────────────────┐
│                    OBSERVABILITY PIPELINE                   │
├────────────────────────────────────────────────────────────┤
│  Workers → OTel Collector (sidecar) → Kafka/NATS           │
│       ↓                                                     │
│  Tempo (traces) ──┬── Grafana (dashboards)                 │
│  Prometheus (metrics) ──┬── Alertmanager (pagerduty)       │
│  Loki (logs) ──────┘                                      │
└────────────────────────────────────────────────────────────┘
```

**Key metrics**:
- `nexus_worker_online` (gauge)
- `nexus_task_duration_seconds` (histogram, by model/shard/worker)
- `nexus_kv_transfer_bytes` (counter, inter-worker)
- `nexus_attestation_failure_total` (counter)
- `nexus_reward_tokens_distributed` (counter)

---

## 4. Model Sharding & Inference Pipeline

### 4.1 Sharding Strategies

#### Pipeline Parallelism (Primary)
```
Input → [Layers 0-15] → [Layers 16-31] → [Layers 32-47] → [Layers 48-63] → Output
         Worker A          Worker B          Worker C          Worker D
              │               │               │               │
              └───────────────┴───────────────┴───────────────┘
                              │
                        KV Cache Transfer
                        (WebRTC DataChannel)
```

**Micro-batching**: Overlap compute + communication
```
Time →
Worker A: [Fwd 1] [Fwd 2] [Fwd 3] [Fwd 4] ...
Worker B:       [Fwd 1] [Fwd 2] [Fwd 3] ...
Worker C:             [Fwd 1] [Fwd 2] ...
Worker D:                   [Fwd 1] ...
```

#### Tensor Parallelism (Intra-layer)
```
Attention Head Split (across 4 workers):
  QKV Projection → [Head 0-7] [Head 8-15] [Head 16-23] [Head 24-31]
                       │          │          │          │
                    All-Reduce (Ring) over WebRTC
                       │          │          │          │
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

### 4.2 KV Cache Management

**Challenge**: KV cache grows with context length (2.8 KB/token for 70B)

**Solutions**:
1. **Coordinator aggregation**: Dedicated node merges KV from pipeline stages
2. **Compression**: 
   - KV quantization (INT8/FP8) — 2-4x reduction
   - Token dropping (H2O, SnapKV) — retain attention sinks + recent
   - Low-rank projection (KIVI) — 8x reduction
3. **Offloading**: CPU RAM ↔ VRAM swapping with async prefetch
4. **P2P distribution**: Shard KV across workers (each holds subset of layers)

### 4.3 Speculative Decoding (Latency Optimization)
```
┌─────────────────────────────────────────────────────────────┐
│                    SPECULATIVE DECODING                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Draft Model (Worker E - CPU/Edge)                          │
│  ├─ Generates 4-8 tokens speculatively                      │
│  └─ Sends draft tokens + KV to Target Model                 │
│                                                             │
│  Target Model (Worker A-D - GPU Pipeline)                   │
│  ├─ Verifies all draft tokens in parallel (single forward)  │
│  ├─ Accepts prefix until first mismatch                     │
│  └─ Generates correction token if needed                    │
│                                                             │
│  Speedup: 2-3x for typical chat workloads                   │
│  Overhead: ~15% extra compute on draft model                │
└─────────────────────────────────────────────────────────────┘
```

### 4.4 Dynamic Resharding
**Triggers**:
- Worker joins/leaves (churn)
- Performance degradation (thermal throttling, OOM)
- New model version deployed
- Load imbalance detected

**Protocol**:
1. Scheduler detects trigger → computes new plan
2. Broadcasts `ReshardCommand` via GossipSub
3. Workers checkpoint KV cache + state
4. Coordinated swap (2-phase commit)
5. Resume from checkpoint

---

## 5. Networking, Security & Incentive Layers

### 5.1 Network Protocol Stack
```
Application Layer: Nexus Protocol (Protobuf over WebSocket/QUIC)
    │
    ├── Control Plane: Task dispatch, heartbeats, manifests, settlement
    ├── Data Plane: KV cache transfer, tensor shards, activations
    └── Management: Attestation, config updates, software updates
    │
Transport Layer: QUIC (primary) / WebRTC DataChannels / WebSocket
    │
    ├── Multiplexing: Multiple streams per connection
    ├── Congestion Control: BBR v3
    └── 0-RTT Resumption: For reconnection after brief disconnect
    │
Security Layer: Noise XX (libp2p) + DTLS 1.3 (WebRTC)
    │
    ├── Mutual Authentication: Ed25519 keys (hardware-bound)
    ├── Forward Secrecy: Ephemeral X25519 per session
    └── Post-Quantum: Hybrid KEM (X25519 + Kyber768) — optional
    │
Network Layer: libp2p (GossipSub, DHT, Identify, Ping)
    │
    ├── Discovery: mDNS (LAN), DHT (WAN), Bootstrap nodes
    ├── NAT Traversal: ICE + STUN + TURN (incentivized)
    └── Relay: Circuit relay v2 (incentivized relay nodes)
```

### 5.2 Security Model

#### Threat Model
| Threat | Mitigation |
|--------|------------|
| Malicious worker returns wrong output | Redundant execution (2x) + challenge sampling |
| Worker steals model weights | TEE encryption, weights never in host memory plaintext |
| Worker leaks user prompts | Input encryption via TEE-derived keys |
| Sybil attack (fake workers) | Hardware-bound identity + stake requirement |
| Coordinator censorship | Multi-coordinator consensus, slashing for censorship |
| Network partition | Local queue + eventual consistency, graceful degradation |
| Model poisoning | Signed manifests, Merkle verification, reproducible builds |

#### Zero-Knowledge Proof of Compute
```
Circuit: RISC Zero / SP1 / Custom (Halo2)
Inputs: 
  - Shard manifest hash
  - Input tensor hash
  - Output tensor hash
  - Execution trace (compressed)
  - TEE quote (public inputs)

Constraints:
  1. Manifest hash matches registered model
  2. Execution trace valid for given architecture
  3. Output hash matches claimed output
  4. TEE quote verifies against known TCB
  5. Resource usage within declared bounds

Proof size: ~100-500 KB (Groth16) / ~50-100 KB (PLONK)
Verification: ~5-10ms on-chain / ~1ms off-chain
```

### 5.3 Incentive Mechanism Design

#### Reward Function
```
Reward(base, worker, task) = base_reward × quality_multiplier × capability_multiplier × uptime_multiplier

base_reward = token_per_flop × task_flops × (1 + priority_fee)
quality_multiplier = 0.5 + 0.5 × (correct_samples / total_samples)  // via challenge
capability_multiplier = log2(vram_gb) / log2(max_vram) × backend_efficiency
uptime_multiplier = min(1.0, uptime_hours / 720)  // caps at 30 days
```

#### Slashing Conditions
| Condition | Slash % | Detection |
|-----------|---------|-----------|
| Failed attestation | 10% stake | TEE quote verification |
| Incorrect output (challenge) | 25% stake | Redundant execution |
| Incorrect output (ZK proof fail) | 50% stake | On-chain verification |
| Extended downtime (>1hr) | 5%/hr | Heartbeat missing |
| Censorship (refuse valid task) | 15% stake | Multi-coordinator audit |

#### Economic Parameters (Initial)
- **Token**: NEXUS (ERC-20 on Arbitrum / SPL on Solana)
- **Min stake**: 10,000 NEXUS (~$500 at launch)
- **Target APR**: 15-25% for honest operators
- **Inflation**: 5% annually, declining to 2%
- **Protocol fee**: 2% of inference spend → treasury

---

## 6. Implementation Roadmap

### Phase 1: Foundation (Months 1-3) — **MVP**
**Goal**: Single-model (LLaMA-3-8B) inference on 2-4 homogeneous GPUs

| Week | Deliverable |
|------|-------------|
| 1-2 | Nexus Agent skeleton (Rust): registration, heartbeat, basic task pull |
| 3-4 | CUDA Shard Executor: pipeline parallelism for 2 GPUs |
| 5-6 | Orchestrator: API Gateway + simple scheduler (round-robin) |
| 7-8 | Model Registry: IPFS storage + manifest format |
| 9-10 | P2P: libp2p GossipSub for heartbeats + task dispatch |
| 11-12 | Integration test: 8B model across 4×RTX 3090 |
| 13-14 | Benchmarks + documentation |

**Success criteria**: 
- 8B model, 4×RTX 3090, >15 tok/s, <2s TTFT
- 99% task success rate over 1000 requests

### Phase 2: Heterogeneity & Security (Months 4-6)
**Goal**: Multi-architecture support + TEE + basic incentives

| Month | Deliverable |
|-------|-------------|
| 4 | Metal/MLX executor (Apple Silicon), llama.cpp executor (CPU) |
| 4 | WebRTC DataChannel integration for KV transfer |
| 5 | SGX/SEV attestation flow + TEE execution mode |
| 5 | ZK proof-of-compute (RISC Zero) for critical paths |
| 6 | Settlement engine (off-chain + on-chain bridge) |
| 6 | Staking/slashing contracts (Arbitrum) |

**Success criteria**:
- 70B model across 8×heterogeneous nodes (NVIDIA + AMD + Apple + CPU)
- TEE attestation success rate >99%
- End-to-end settlement latency <5 min

### Phase 3: Advanced Sharding & Scale (Months 7-10)
**Goal**: Production-grade sharding, MoE support, speculative decoding

| Month | Deliverable |
|-------|-------------|
| 7 | Tensor parallelism + expert parallelism |
| 7 | Dynamic resharding protocol |
| 8 | KV cache compression (INT8, KIVI, H2O) |
| 8 | Coordinator node role + KV aggregation |
| 9 | Speculative decoding (draft model on edge) |
| 9 | Ring attention for long context (128k+) |
| 10 | Load testing: 100+ concurrent workers, 400B model |

**Success criteria**:
- 400B MoE model (Mixtral-style) across 32 workers
- Speculative decoding 2.5x speedup
- 99.9% uptime SLA for coordinators

### Phase 4: Network Effects & Ecosystem (Months 11-15)
**Goal**: Self-sustaining network, developer platform, mobile/edge

| Month | Deliverable |
|-------|-------------|
| 11 | Mobile SDK (Android/iOS) + NPU executors |
| 11 | Browser WASM worker (WebGPU + WebLLM) |
| 12 | Developer portal: model onboarding, analytics, billing |
| 12 | Marketplace: custom models, fine-tunes, LoRAs |
| 13 | Federation: multi-region orchestrators, cross-cluster |
| 13 | Governance: on-chain parameter updates, DAO |
| 14 | Enterprise features: SLA, dedicated coordinators, VPC peering |
| 15 | Mainnet launch + token generation event |

### Phase 5: Research & Optimization (Ongoing)
- **Communication-optimal sharding**: 3D parallelism, hierarchical all-reduce
- **Privacy-preserving inference**: FHE for sensitive prompts, MPC for secret sharing
- **Model routing**: Learned router for MoE + cascade (small→large)
- **Hardware co-design**: Collaboration with AMD/Intel/Apple for LLM-specific features

---

## 7. Technical Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Network latency kills pipeline throughput | High | High | Aggressive micro-batching, KV compression, coordinator aggregation |
| Worker churn during long generation | High | Medium | Checkpointing every 10 layers, fast resharding (<5s) |
| TEE not available on consumer hardware | Medium | High | Graceful degradation: non-TEE mode with reputation-based trust |
| ZK proof generation too slow | Medium | High | Offload to dedicated prover network, use faster systems (SP1) |
| Model weight distribution bandwidth | Medium | Medium | BitTorrent-style swarm download, delta updates, CDN cache |
| Regulatory pressure on compute markets | Low | High | Jurisdiction-aware routing, compliance modes, audit logs |
| Sybil attacks on worker registry | Medium | High | Hardware-bound identity (TPM), stake + slashing, reputation |

---

## 8. Repository Structure (Monorepo)

```
nexusllm/
├── crates/
│   ├── nexus-agent/          # Worker daemon (Rust)
│   ├── nexus-executor/       # Shard executor trait + backends
│   │   ├── cuda-backend/
│   │   ├── metal-backend/
│   │   ├── rocm-backend/
│   │   ├── llama-cpp-backend/
│   │   └── wasm-backend/
│   ├── nexus-tee/            # TEE abstraction (SGX/SEV/TrustZone/SE)
│   ├── nexus-p2p/            # libp2p wrapper + WebRTC integration
│   ├── nexus-scheduler/      # DAG scheduler + cost model
│   ├── nexus-registry/       # Model + Worker registry clients
│   ├── nexus-settlement/     # On-chain + off-chain settlement
│   ├── nexus-proto/          # Protobuf definitions
│   └── nexus-common/         # Shared types, crypto, metrics
├── orchestrator/
│   ├── api-gateway/          # Axum/Go HTTP + WebSocket
│   ├── scheduler-service/    # Scheduler gRPC service
│   ├── registry-service/     # Worker/Model registry
│   ├── settlement-service/   # Settlement processor
│   └── observability/        # OTel collector config, dashboards
├── contracts/
│   ├── staking/              # Solidity/Rust staking contracts
│   ├── rewards/              # Reward distribution
│   ├── slashing/             # Slashing logic
│   └── governance/           # DAO parameters
├── sdk/
│   ├── python/               # Python client SDK
│   ├── typescript/           # TypeScript/JS SDK
│   ├── go/                   # Go SDK
│   └── rust/                 # Rust SDK
├── cli/
│   └── nexusctl/             # CLI for operators/developers
├── docs/
│   ├── architecture/
│   ├── api/
│   ├── deployment/
│   └── economics/
├── benchmarks/
│   └── ...
├── tests/
│   ├── integration/
│   ├── chaos/
│   └── load/
├── docker/
│   ├── worker/
│   ├── coordinator/
│   └── orchestrator/
├── helm/
│   └── nexusllm/             # Kubernetes deployment
├── Cargo.toml                # Workspace root
├── turbo.json                # Turborepo config
└── README.md
```

---

## 9. Quick Start (Developer)

```bash
# 1. Clone & build
git clone https://github.com/nexusllm/nexusllm
cd nexusllm
cargo build --release --workspace

# 2. Start local orchestrator (dev mode)
docker-compose -f docker/dev.yml up -d

# 3. Register as worker
./target/release/nexus-agent \
  --orchestrator ws://localhost:8080 \
  --model-cache-dir ./cache \
  --tee-mode auto

# 4. Submit inference request
curl -X POST http://localhost:8080/v1/inference \
  -H "Authorization: Bearer $API_KEY" \
  -d '{"model": "llama-3-8b-instruct", "prompt": "Hello, distributed world!", "stream": true}'
```

---

## 10. Conclusion

NexusLLM transforms the world's idle compute into a planetary-scale inference fabric. By combining **heterogeneous hardware abstraction**, **TEE-secured execution**, **P2P data-plane efficiency**, and **crypto-economic alignment**, it achieves data-center performance at a fraction of the cost — while preserving user privacy and model IP.

The architecture is designed for **progressive decentralization**: starting with a permissioned orchestrator for quality bootstrapping, evolving toward a fully permissionless, DAO-governed network where anyone can contribute compute and earn rewards proportional to their verifiable contribution.

**Next steps**: Begin Phase 1 implementation with core team. Target testnet launch at Month 6, mainnet at Month 15.