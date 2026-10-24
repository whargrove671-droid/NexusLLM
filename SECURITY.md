# NexusLLM Security Model

## Security Guarantees

NexusLLM provides three core guarantees, each cryptographically enforced:

| Guarantee | Mechanism | Verification |
|-----------|-----------|--------------|
| **Prompt Privacy** | End-to-end encryption + TEE execution | TEE remote attestation |
| **Output Correctness** | Redundant execution + ZK proof-of-compute | On-chain proof verification |
| **Weight Confidentiality** | Weights only exist decrypted inside TEEs | Attestation + sealed storage |

**Security Property**: A user can verify, with cryptographic certainty, that their inference was (a) executed correctly, (b) on the genuine model, (c) without the operator seeing the plaintext prompt or output — without trusting any individual worker.

---

## 1. Threat Model

### Adversary Classes

| Adversary | Capabilities | Assumed Resource Limit |
|-----------|--------------|------------------------|
| **Malicious Worker** | Full control of one node OS, kernel, hypervisor; arbitrary network behavior | Cannot break TEE hardware, cannot break Ed25519/AES-GCM |
| **Malicious Operator Cartel** | Controls up to 30% of network stake | Cannot exceed 1/3 of workers for any single task assignment |
| **Network Adversary** | Passive eavesdropping, active MITM on all links, DNS manipulation | Cannot break TLS 1.3 / Noise XX |
| **Malicious Coordinator** | Controls one coordinator node | Limited by multi-coordinator consensus (2-of-3) |
| **Malicious User** | Submit adversarial prompts, refuse payment, abuse workers | Rate limits + pre-payment escrow |
| **Supply-Chain Adversary** | Compromise a released binary or dependency | Reproducible builds + signed releases + runtime integrity checks |

### Out of Scope

- Physical extraction of keys from TEE hardware (assumed infeasible per vendor security claims)
- Side-channel attacks on specific TEE implementations (mitigated where vendor patches exist; residual risk disclosed)
- Quantum adversaries today (hybrid PQ key exchange available as opt-in)

---

## 2. TEE Security Architecture

### 2.1 Platform Matrix

| Platform | Enclave | Attestation Protocol | Memory Protection | Min TCB |
|----------|---------|---------------------|-------------------|---------|
| Intel SGX | EPC enclave | DCAP (ECDSA quote) | MEE (AES-GCM, integrity tree) | CPU package + PCE |
| AMD SEV-SNP | Encrypted VM | SNP report (VCEK-signed) | AES-128 SME + RMP integrity | PSP + VMPL0 |
| ARM TrustZone | Trustlet/TEE OS | DICE attestation chain | TZASC memory partitioning | Boot ROM + TEE OS |
| Apple Secure Enclave | SEP coprocessor | App Attest (DeviceCheck) | Dedicated crypto processor | SEP firmware |
| Software fallback | Process isolation | None (self-signed) | OS page protection | Full OS + app |

**Policy**: Tasks carrying user data are only assigned to hardware TEE workers. Software-fallback workers receive only: (a) benchmark/synthetic tasks, (b) public-model warmup, (c) non-sensitive batch jobs explicitly opted in by the user.

### 2.2 Attestation Flow (Detailed)

```
Worker (inside TEE)                     Orchestrator
        │                                    │
        │  1. Generate ephemeral P-256       │
        │     keypair inside enclave         │
        │                                    │
        │  2. Compute MRENCLAVE /            │
        │     measurement of executor binary │
        │                                    │
        │ ── 3. Quote{measurement, ────────► │
        │       ephemeral_pubkey,            │
        │       TCB version, timestamp}      │
        │       signed by platform key       │
        │                                    │
        │                     4. Verify quote chain
        │                        against Intel/AMD/ARM/Apple
        │                        root certificates
        │                                    │
        │                     5. Check TCB version ≥
        │                        minimum accepted
        │                        (revocation list)
        │                                    │
        │                     6. Verify measurement ∈
        │                        whitelist of approved
        │                        executor builds
        │                                    │
        │ ◄── 7. Orchestrator ephemeral ───── │
        │       pubkey + acceptance          │
        │                                    │
        │  8. ECDH(ephemeral keys) ────────► 8. ECDH
        │     → session key (AES-256-GCM)    │     → session key
        │     bound to quote hash            │     bound to quote hash
        │                                    │
        ▼                                    ▼
```

**Key properties**:
- Session key exists only inside the TEE and in the orchestrator's HSM-backed memory
- Key is bound to the specific enclave measurement — a patched executor gets a different measurement → quote rejected
- Quotes expire after 5 minutes; re-attestation is mandatory

### 2.3 What Runs Inside the Enclave

```
┌────────────────────────────────────────────┐
│              TEE Enclave                   │
├────────────────────────────────────────────┤
│  ✓ Model weights (decrypted on load)       │
│  ✓ Forward pass computation                │
│  ✓ KV cache                                │
│  ✓ Prompt/output plaintext                 │
│  ✓ Session key                             │
│  ✓ ZK proof witness generation             │
├────────────────────────────────────────────┤
│  ✗ Network stack (outside, encrypted I/O)  │
│  ✗ Disk access (sealed storage API only)   │
│  ✗ Logging (outside, sees ciphertext only) │
└────────────────────────────────────────────┘
```

### 2.4 Sealed Storage

Model weights are cached locally encrypted with a key derived from the enclave's sealing key:

- **SGX**: `EGETKEY` with MRENCLAVE policy — only the same executor build can unseal
- **SEV-SNP**: VMPL-isolated encrypted pages + PSP-derived key
- **Apple**: Secure Enclave keychain with `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
- **TrustZone**: RPMB secure storage partition

An attacker who copies the cache directory gets ciphertext tied to hardware they don't possess.

---

## 3. Zero-Knowledge Proof-of-Compute

### 3.1 Why ZK (Not Just Redundancy)

Redundant execution (2-3 workers per task) catches *random* cheating but not *adaptive* cheating (a worker that answers challenges correctly but free-rides on cache hits). ZK proofs make correctness unconditionally verifiable.

### 3.2 Proof System

| Property | Value |
|----------|-------|
| Proving system | RISC Zero zkVM (STARK→Groth16 compression) |
| Alternative | SP1 (PLONK-based) for recursion |
| What's proven | Forward pass on committed weights produces committed output for committed input |
| Proof size | ~200 KB (Groth16-compressed) |
| Prove time | ~2-4× execution time (amortized via batch proving) |
| Verify time (on-chain) | ~250k gas on Arbitrum (~5ms) |
| Verify time (off-chain) | <1ms |

### 3.3 What's in the Public Statement

```
public_inputs = {
    model_manifest_hash,   // binds to specific model + version
    shard_id,              // which layers
    input_commitment,      // Poseidon hash of input tensor
    output_commitment,     // Poseidon hash of output tensor
    tee_quote_hash,        // binds proof to attested enclave
    epoch,                 // replay protection
}
```

The witness (full tensors, activations) never leaves the TEE. The orchestrator receives the proof + commitments and verifies without seeing plaintext.

### 3.4 Probabilistic Proof Sampling

Full proving on every token is uneconomical. We use:

| Mode | Coverage | When |
|------|----------|------|
| **Full proof** | 100% of tasks | HIGH/CRITICAL priority, enterprise tier |
| **Sampled proof** | 1% random tasks | NORMAL priority |
| **Challenged proof** | On-demand | Any verifier disputes a result (fraud window: 7 days) |
| **Redundant execution** | 5% of tasks cross-checked | Random sampling, outcomes compared via commitment equality |

A worker caught by either mechanism is slashed 25-50% and ejected. Expected value of cheating is strictly negative:

```
EV(cheat) = P(caught) × penalty − P(uncaught) × reward
          = 0.06 × 0.50 × stake − 0.94 × reward_per_task
          ≪ 0   for stake ≥ 10,000 NEXUS
```

---

## 4. Network Security

### 4.1 Transport

| Channel | Protocol | Cipher Suite |
|---------|----------|--------------|
| Client → Orchestrator | TLS 1.3 | TLS_AES_256_GCM_SHA384, X25519 |
| Worker ↔ Worker | libp2p Noise XX | ChaCha20-Poly1305, X25519 |
| KV cache transfer | WebRTC DTLS 1.3 | AES-256-GCM |
| Inside TEE boundary | AES-256-GCM session keys | Per-task ephemeral |

### 4.2 Identity

- **Worker identity**: Ed25519 keypair, public key derived from TPM/EK endorsement → hardware-bound, non-transferable
- **Peer ID**: `SHA-256(worker_pubkey)` → libp2p PeerID
- **Sybil cost**: Each identity requires distinct hardware attestation + 10,000 NEXUS stake

### 4.3 DDoS Resistance

- QUIC with connection migration + retry tokens
- Coordinator layer absorbs volumetric attacks (Anycast + cloud scrubbing)
- Workers never expose public listener ports (NAT via relay/WebRTC only)
- GossipSub peer scoring: invalid messages, flooding → score decay → mesh ejection

---

## 5. Scheduler & Assignment Security

### 5.1 Assignment Unpredictability

Task→worker assignment uses verifiable random selection:

```
seed = VRF(scheduler_key, task_id || epoch)
eligible = workers_matching(capability_requirements)
selected = sort_by(VRF_score(seed, worker_id), eligible)[:num_stages]
```

A cartel cannot predict or influence which workers receive which shards — preventing targeted collusion on specific user traffic.

### 5.2 Prompt Splitting (Defense in Depth)

For enterprise tier, prompts can be split so no single worker sees the full context:

- **Tensor-parallel tasks**: each worker sees only its attention-head slice activations, never the full embedding
- **Prefix concealment**: system prompt tokenized and processed by a separate high-trust enclave; only KV state is forwarded

### 5.3 Coordinator Byzantine Tolerance

- 3 coordinators per task (different operators, different regions)
- 2-of-3 must agree on: task completion, worker output commitments, reward attribution
- A censoring coordinator is detected by the other two → slashed

---

## 6. Smart Contract Security

### 6.1 Contract Suite

| Contract | Lines | Audited | Key Risk |
|----------|-------|---------|----------|
| `Staking.sol` | ~800 | ✓ | Reentrancy on unstake |
| `Rewards.sol` | ~600 | ✓ | Integer precision in pro-rata math |
| `Slashing.sol` | ~400 | ✓ | Governance capture of slash votes |
| `Delegation.sol` | ~500 | ✓ | stNEXUS depeg under mass slashing |
| `ZKVerifier.sol` | ~300 | ✓ | Trusted setup integrity (Groth16) |

### 6.2 Contract Hardening

- OpenZeppelin libraries only (v5.x)
- Checks-Effects-Interactions on all state mutations
- `ReentrancyGuard` on all external-call paths
- Time-locked parameter changes (48h) via governance
- Emergency pause (multisig 3-of-5, expires after 72h — cannot freeze funds permanently)
- Upgradeability: UUPS proxies, upgrades gated by 7-day timelock + DAO vote

### 6.3 Audits

| Date | Firm | Scope | Result |
|------|------|-------|--------|
| TBD | Trail of Bits | Full contract suite | Pre-launch requirement |
| TBD | Zellic | ZK verifier + staking | Pre-launch requirement |
| Quarterly | Internal + community | Regression | Ongoing |

**Bug Bounty**: Up to $250,000 for critical contract vulnerabilities, $50,000 for TEE bypass, $25,000 for scheduler manipulation. Program at `security.nexusllm.network`.

---

## 7. Client-Side Security

### 7.1 Runtime Integrity (CLI/SDK)

- SHA-384 module hash verification at startup (`integrity.ts` pattern)
- Signed releases: minisign + reproducible builds; hash published on-chain
- CSP + Trusted Types for web UI
- No third-party telemetry; all metrics are opt-in, anonymized, and inspectable

### 7.2 Key Storage (User API Keys)

- OS keychain (macOS Keychain, Windows DPAPI, libsecret)
- Never written to config files or environment dumps
- Keys scoped: `inference` keys cannot withdraw funds; `billing` keys cannot read prompts

### 7.3 Operator Key Storage

- Stake keys: hardware wallet strongly recommended
- Operator hot key (for heartbeat signing): sealed in TEE, distinct from stake key
- Key compromise path: hot key rotation without touching stake (see Operator Guide §Key Rotation)

---

## 8. Privacy Analysis

### 8.1 What Each Party Sees

| Party | Sees | Does NOT See |
|-------|------|--------------|
| User | Everything (it's their data) | — |
| Orchestrator | Model ID, token counts, timing, ciphertext | Prompt plaintext, output plaintext, weights |
| Worker | Its shard's activations (inside TEE only) | Other shards' activations, full weights of other shards, user identity |
| Coordinator | Commitments, proofs, metadata | Any plaintext tensor data |
| Blockchain observer | Task IDs, reward flows, stake movements | Any link to prompt content |

### 8.2 Metadata Resistance (Residual Risks)

Honest disclosure of what we do *not* fully protect:

| Leak | Mitigation | Residual Risk |
|------|-----------|---------------|
| Token count reveals prompt length | Padding to bucket sizes (opt-in) | Buckets leak O(log n) bits |
| Timing reveals model + rough length | Constant-rate streaming option | Higher latency |
| Traffic volume per user | Not mitigated in v1 | Use VPN/Tor for network-layer privacy |
| Worker learns its shard exists | Inherent to architecture | No full-model knowledge per worker |

**Roadmap**: FHE-based prefix processing (Phase 5) for fully-blind prompt handling on high-trust enclave clusters.

---

## 9. Incident Response

### 9.1 Severity Levels

| Level | Definition | Response Time | Examples |
|-------|-----------|---------------|----------|
| P0 | Funds at risk, TEE broken, weights extracted | 1 hour | Enclave escape CVE, contract exploit |
| P1 | Incorrect outputs at scale, mass slashing bug | 4 hours | Scheduler corruption, bad model manifest |
| P2 | Privacy degradation, attestation bypass | 24 hours | Quote forgery, side channel |
| P3 | DoS, performance attacks | 72 hours | GossipSub flooding |

### 9.2 Playbooks

**P0 — TEE Compromise**:
1. Revoke TCB version in orchestrator whitelist (< 15 min)
2. Reject all quotes from affected TCB level
3. Rotate session keys network-wide
4. Force executor binary update (new measurement)
5. Post-mortem + disclosure within 72h

**P0 — Contract Exploit**:
1. Emergency pause (multisig, < 1 hour)
2. Snapshot pre-exploit state
3. DAO vote on remediation (compensate / migrate)
4. New audit before unpause

### 9.3 Disclosure Policy

- **Responsible disclosure**: security@nexusllm.network (PGP key on website)
- **Embargo**: 90 days standard, negotiable for active exploits
- **Credit**: Public acknowledgment (unless anonymity requested) + bounty

---

## 10. Security Configuration Reference

### User-Side Hardening

```toml
# ~/.config/nexus/config.toml — maximum privacy profile
[privacy]
require_tee = true              # Reject software-fallback workers
verify_zk_proofs = true         # Verify every proof client-side
require_full_proofs = true      # No sampling — proof on every task (higher cost)
pad_token_counts = true         # Bucket padding
constant_rate_streaming = true  # Timing resistance
audit_log = true
audit_log_retention_days = 365
```

### Operator-Side Hardening

```toml
# operator.toml — security profile
[security]
tee_required = true
hot_key_in_tee = true
attestation_interval_min = 5
max_task_age_seconds = 3600
reject_unattested_peers = true
min_peer_tcb_version = "latest"
```

### Verification Commands

```bash
# Verify a task's full security chain
nexus verify --task <task_id> --full
# Checks: TEE quote → measurement whitelist → ZK proof → output commitment

# Verify your client binary
nexus self-check --integrity

# Check network TCB health
nexus security tcb-status
```

---

## 11. Cryptographic Inventory

| Primitive | Algorithm | Usage | Standard |
|-----------|-----------|-------|----------|
| Identity signatures | Ed25519 | Worker/operator identity | RFC 8032 |
| Key exchange | X25519 (ECDH) | Session keys | RFC 7748 |
| Session encryption | AES-256-GCM | Task I/O | NIST SP 800-38D |
| P2P encryption | ChaCha20-Poly1305 | Noise XX transport | RFC 8439 |
| Commitments | Poseidon | ZK-friendly tensor hashing | Grassi et al. |
| Hashing | SHA-256 / SHA-384 | Manifests, integrity | FIPS 180-4 |
| Key derivation | HKDF-SHA256 | Sealing, session chains | RFC 5869 |
| ZK proofs | STARK→Groth16 | Compute correctness | RISC Zero |
| VRF | ECVRF (secp256k1) | Assignment randomness | RFC 9381 |
| PQ (opt-in) | X25519+Kyber768 hybrid | Future-proofing sessions | NIST FIPS 203 |

---

*Security is a process, not a state. This document is versioned alongside the codebase; report discrepancies to security@nexusllm.network.*
