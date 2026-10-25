# NexusLLM — Codebase Audit & Roadmap

**Audit date:** October 25, 2026 · **Branch:** `main` (`866e049`)
**Scope:** 9-crate Rust workspace + proto + docker/helm + documentation (43 tracked files, 3,906 lines of `.rs` + proto)

---

## Part 1 — Codebase Audit

### 1.1 State Summary by Crate

| Crate | `.rs` LOC | Compile Errors | Status | Assessment |
|-------|-----------|:----------:|--------|-----------|
| `nexus-proto` | 1,685 | **0** | ✅ Compiles | Generated protobuf + build.rs. **Complete.** |
| `nexus-common` | 182 | **0** | ✅ Compiles | Errors, crypto, serialization, metrics, time, tensor. **Complete & usable.** |
| `nexus-tee` | 656 | **91** | ❌ Broken | Large trait + 5 backends written, but `TEEBackend` not dyn-compatible; missing `aes_gcm`/`blake3`/`rand` deps. **Partial — needs refactor.** |
| `nexus-executor` | 603 | **15** | ❌ Broken | `ShardExecutor` trait + 5 backend impls written; `Shape::From<&Vec<usize>>` + generic-arg errors. **Partial — needs API fix.** |
| `nexus-agent` | 586 | **119** | ❌ Broken | `config`/`task_runner`/`heartbeat`/`attestation` modules **declared but files don't exist**. Heaviest-breakage; mostly cascading from missing submodules. |
| `nexus-p2p` | 54 | **8** | ❌ Skeleton | `todo!()` bodies; missing `behaviour`/`transport`/`rpc` modules. **Scaffold only.** |
| `nexus-scheduler` | 22 | **5** | ❌ Skeleton | `todo!()` bodies; missing `dag`/`planner`/`cost_model`. **Scaffold only.** |
| `nexus-registry` | 42 | **7** | ❌ Skeleton | `todo!()` bodies; missing `model_registry`/`worker_registry`. **Scaffold only.** |
| `nexus-settlement` | 30 | **8** | ❌ Skeleton | `todo!()` bodies; missing `ethereum`/`solana`/`zk`/`rpc`. **Scaffold only.** |

**Totals:** 3,906 LOC `.rs`+proto, **134 compile errors**, 6 of 9 crates fail `cargo check`.

### 1.2 Crate Classification

- **Live & compiling (2):** `nexus-proto`, `nexus-common`
- **Substantial but non-compiling (3):** `nexus-tee`, `nexus-executor`, `nexus-agent` — real (often ~600 LOC) designs written, but with API/dep drift so they don't build. Highest-leverage fix targets.
- **Skeleton stubs (4):** `nexus-p2p`, `nexus-scheduler`, `nexus-registry`, `nexus-settlement` — method declarations with `todo!()` bodies and missing module files. Require from-scratch implementation.

### 1.3 Root-Cause Error Classes

| Error | Count | Where | Fix |
|-------|:-----:|-------|-----|
| `E0107` enum needs 2 generic args | 67 | proto-generated code | Regenerate with matching `prost` version; align `candle` Shape generics |
| `E0038` trait not dyn-compatible | 27 | `nexus-tee` | `TEEBackend` has `async`/generic methods — split into boxed or replace `async` with channel-based API |
| `E0583` module file not found | 15 | agent/p2p/scheduler/registry/settlement | Create declared submodule files or remove dead `pub mod` declarations |
| `E0407`/`E0433` method/import drift | ~15 | Multiple | Align trait surface; add missing deps |
| Dep drift (`aes_gcm`, `blake3`, `hmac`, `rand`, `sha2`, `async_trait`) | ~6 | tee/executor/common | Add missing crate deps to manifests |
| `E0599` method not on trait | 1 | tee manager | Use `execute_in_tee` via trait not concrete impl |

### 1.4 Security & Hygiene (re-verified)

| Check | Result |
|-------|--------|
| Secrets / credentials in code | ✅ None (doc refs are placeholders only) |
| `unsafe` blocks | ✅ Zero |
| External network calls in crates | ✅ None (deferred to orchestrator, correctly) |
| `.gitignore` effectiveness | ✅ `target/`, `protoc*`, `*.exe`, `.env`, `model_cache/` excluded |
| License metadata vs LICENSE file | ✅ All 9 crates now `LicenseRef-NexusLLM-1.0` |
| Repository URLs | ✅ Corrected to `whargrove671-droid/NexusLLM` |
| **New finding:** stale SGX dep broke manifest | ⚠️ **Found & fixed this audit** (`866e049`) |
| `.unwrap()` on protobuf/`SystemTime` in common | ⚠️ Low — acceptable for prototype, add `expect()` before production |

### 1.5 Key Risks / Gaps (must-address before MVP)

1. **Nothing is runnable end-to-end.** `nexus-common` + `nexus-proto` compile, but no orchestration path exists; all 4 skeleton crates are `todo!()`.
2. **No tests** beyond a handful of unit tests in `nexus-common` & `nexus-agent` (which won't run because agent doesn't compile).
3. **No CI** — no `.github/workflows`, so `cargo check`/clippy/test never run automatically. Regressions like the C3-breakage go undetected.
4. **Toolchain drift** — manifests reference crates (`sgx-types`/`sgx-urts` 2019, `candle`/`prost` versions) that don't align with the written code.
5. **No orchestrator services** — the `/orchestrator` dir referenced in docs does not exist in the repo; API gateway, scheduler service, settlement service all unimplemented.
6. **No docker/helm live assets** — compose/helm exist as YAML but nothing runs against them.
7. **Design-doc ↔ code mismatch** — README/docs describe a shipped product; the skeleton banner now corrects this, but code still implies more than exists.

---

## Part 2 — Roadmap

### Guiding Principle

**Make it compile, then make it run, then make it trustworthy.** Each phase is independently verifiable with a real gate (a CI check or a live binary) — not "planning".

---

### Phase 0 — Stabilize the Foundation *(Week 1–2)*
**Goal:** Whole workspace compiles; CI catches regressions.

- [ ] Fix the 3 non-compiling real crates (`nexus-tee`, `nexus-executor`, `nexus-agent`):
  - [ ] Regenerate `nexus-proto` with aligned `prost` → resolves the 67 `E0107` errors
  - [ ] Make `TEEBackend` dyn-compatible (box futures / channel API) → 27 errors + `execute_in_tee`
  - [ ] Create the 4 missing `nexus-agent` submodule files (`config`, `task_runner`, `heartbeat`, `attestation`) or prune the declarations
  - [ ] Add missing deps (`aes_gcm`, `blake3`, `hmac`, `rand`, `sha2`) to correct manifests
- [ ] Add **CI**: GitHub Actions `ci.yml` running `cargo check`, `cargo clippy -D warnings`, `cargo test`, `cargo fmt --check` on `main` + PRs
- [ ] Gate: ✅ `cargo check --workspace` passes on a clean checkout (CI green)

### Phase 1 — Minimal Viable Inference (MVP) *(Week 3–6)*
**Goal:** One real inference path works on a single machine before distribution.

- [ ] Implement `nexus-executor` CPU backend end-to-end (llama.cpp/GGUF loading → forward pass → sample)
- [ ] Get `nexus-agent` standing up: identity, capability discovery, local executor, run one local task
- [ ] Implement `nexus-settlement` minimally: reward accounting, proof placeholder (no on-chain yet)
- [ ] **Local smoke test:** CLI runs a 1B-model inference on the host CPU; prints tokens; records metrics
- [ ] Add a thin `nexus-cli` binary surface (run / models / status)
- [ ] Gate: ✅ `nexus run llama-1b "hello"` prints tokens with per-token latency on a single machine

### Phase 2 — Trusted Distribution *(Week 7–10)*
**Goal:** Two or more machines cooperate; trust is cryptographically grounded.

- [ ] Implement `nexus-tee` software-fallback + seal/unseal; unit-test cryptography
- [ ] Implement `nexus-p2p` properly: libp2p node init, discovery, one RPC (`register_worker`), NAT (relay/DCUtR)
- [ ] Implement `nexus-scheduler` minimal: single-strategy (pipeline) planning on a 2–4 worker pool
- [ ] Define + implement `nexus-registry` `model_registry` (localhost) + worker heartbeat store
- [ ] **Two-node demo:** one orchestrator node + one worker node; agent registers and runs a shard
- [ ] Gate: ✅ 2-node `docker-compose` topology runs a split pipeline; heartbeat table updates

### Phase 3 — Correctness & Verification *(Week 11–14)*
**Goal:** Outputs checkable; inputs private.

- [ ] Pin provable execution: redundant 2x execution + commitment comparison on sampled tasks
- [ ] `TEEBackend` for one hardware target (SGX on Linux or Secure Enclave on macOS) with real attestation + quote verification
- [ ] ZK proof-of-compute stub → real proof for the CPU path (start with a small circuit: "forward pass executed")
- [ ] Gate: ✅ verifier rejects a tampered result; attestation quote validates against hardware/firmware

### Phase 4 — Incentives & Settlement *(Week 15–18)*
**Goal:** Economic layer live on a testnet.

- [ ] `nexus-settlement` on-chain: staking + rewards contracts (Arbitrum Sepolia testnet)
- [ ] Reward formula engine in Rust (quality/capability/uptime multipliers)
- [ ] Slashing flow: failed attestation → slash; proof-invalid → slash (testnet only)
- [ ] `nexus-scheduler` cost-model + dynamic assignment
- [ ] Gate: ✅ testnet: worker earns testnet NEXUS; a bad worker is slashed; governance can tweak a parameter

### Phase 5 — Scale & Productization *(Month 5+)*
**Goal:** Multi-strategy, observable, near-production.

- [ ] Tensor parallelism + MoE (expert) routing
- [ ] KV-cache transfer over WebRTC DataChannels (the differentiator)
- [ ] Speculative decoding (draft-on-edge → verify on GPU)
- [ ] Observability: Prometheus/OTel dashboards, alerting
- [ ] Full orchestrator services: API gateway (auth/rate-limit), scheduler service, registry service
- [ ] Hardened security: audit, bug-bounty scope, TEE side-channel review, exhaustive `.unwrap()` elimination
- [ ] **Public beta:** open registration, real NEXUS mainnet (subject to token launch decision)
- [ ] Gate: ✅ 10-node cluster runs a 70B model sharded across GPUs; observability shows per-stage latency

### Phase 6 — DAO & Network Effects *(Month 8+)*
- [ ] Governance DAO (parameter voting, treasury)
- [ ] Delegation / liquid staking (`stNEXUS`)
- [ ] Developer portal + model marketplace
- [ ] Mobile/browser (WASM) workers
- [ ] Multi-region federation

---

### Dependency Graph (implementation order)

```
nexus-proto → nexus-common
                  ├──→ nexus-tee ──────────────→ (Phase 0/3)
                  ├──→ nexus-executor ─────────→ (Phase 0/1/5)
                  ├──→ nexus-p2p ──────────────→ (Phase 2)
                  ├──→ nexus-scheduler ───────── → (Phase 2/4)
                  ├──→ nexus-registry ──────────→ (Phase 2)
                  └──→ nexus-settlement ────────→ (Phase 4)
                            └── all feed → nexus-agent (Phase 1)
```

### Critical Path
**`car-go-check` green (P0) → CPU inference works locally (P1) → 2-node cooperation (P2) → outputs verifiable (P3) → testnet settle (P4) → sharded 70B + observability (P5).**

The single biggest unlock is **Phase 0** — until the workspace compiles and CI runs, every later phase is unverifiable. That one phase removes 134 errors, restores test execution, and turns a scaffold into a buildable codebase.

---

*This roadmap is a living document; update phase checkboxes as gates are met. Commit norms: conventional commits, one crate per PR, CI must pass before merge.*