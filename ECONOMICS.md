# NexusLLM Economics Deep-Dive

## Overview

NexusLLM's economic layer aligns three parties:

| Party | Provides | Receives |
|-------|----------|----------|
| **Users** | Payment per token | ~90% cheaper inference than cloud |
| **Operators** | Compute + stake | NEXUS rewards (15-25% target APR) |
| **Delegators** | Capital (stake) | Pro-rata share of operator rewards |

The protocol captures value through a **2% inference fee** and the **20% commercial software royalty** (License §2), both funding the treasury.

---

## 1. The NEXUS Token

### 1.1 Specification

| Property | Value |
|----------|-------|
| Standard | ERC-20 (Arbitrum One) / SPL (Solana) |
| Decimals | 18 |
| Initial supply | 1,000,000,000 NEXUS |
| Max supply | Uncapped (inflationary, decaying) |
| Inflation schedule | 5% Y1 → 4% Y2 → 3% Y3 → 2% terminal |
| Inflation destination | 100% to operator reward pool |
| Protocol fee | 2% of inference spend → treasury |

### 1.2 Initial Distribution

| Allocation | % | Vesting |
|------------|---|---------|
| Community & ecosystem | 40% | 48 months linear |
| Operator bootstrap incentives | 20% | Released against verified compute milestones |
| Team | 15% | 12-month cliff, 36-month linear |
| Investors | 15% | 12-month cliff, 24-month linear |
| Treasury | 8% | DAO-controlled |
| Liquidity | 2% | Immediate (DEX pools) |

### 1.3 Token Utility

1. **Staking** — operators must bond NEXUS to accept tasks
2. **Payment medium** — users may pay in NEXUS (5% discount vs USD-denominated stablecoins)
3. **Governance** — 1 NEXUS = 1 vote on protocol parameters
4. **Delegation** — stakers delegate to operators, receive liquid stNEXUS
5. **Fee capture** — treasury may execute buyback-and-burn (DAO vote)

---

## 2. User-Side Economics

### 2.1 Pricing Model

Price per token is set by a **two-sided market clearing** mechanism:

```
price_per_1k_tokens = max(floor_price, clearing_price)

floor_price = amortized_hardware_cost × 1.3  (30% operator margin floor)
clearing_price = f(demand_queue_depth, eligible_supply, model_size)
```

Published indicative pricing (USD):

| Model Class | Cloud Price/1K | NexusLLM Target/1K | Floor/1K |
|-------------|---------------|--------------------|----|
| 3-8B | $0.0008 | $0.00008 | $0.00005 |
| 13-14B | $0.0015 | $0.00015 | $0.00009 |
| 30-34B | $0.004 | $0.0004 | $0.00025 |
| 70-72B | $0.008 | $0.0008 | $0.0005 |
| 140B+ MoE | $0.03 | $0.004 | $0.002 |
| 400B+ | $0.06 | $0.006 | $0.004 |

### 2.2 Why 90% Cheaper Is Sustainable

Cloud GPU pricing includes: data-center capex, facilities, enterprise margins, idle capacity, and reserved-instance risk. NexusLLM operators already own the hardware — their marginal cost is **electricity + depreciation**:

```
Marginal cost per 1K tokens (RTX 4090, LLaMA-70B sharded):
  Electricity:  450W × 0.0004 kWh/1Ktok × $0.12/kWh  ≈ $0.000022
  Depreciation: $1,600 / 3yr / (31.5M tok/day × 1095 days) ≈ $0.000008
  Bandwidth:    ≈ $0.000005
  ─────────────────────────────────────────
  Total marginal: ≈ $0.000035/1K → floor with margin ≈ $0.00005
```

The 90% discount is real marginal-cost pricing, not subsidy. Subsidy (bootstrap inflation rewards) is layered *on top* for early operators and phases out as fee revenue grows.

### 2.3 Payment Flows

```
User deposits (USDC / NEXUS / fiat→stable onramp)
        │
        ▼
┌─────────────────┐
│  Escrow balance │──► per-task pre-authorization (est. cost × 1.2)
└────────┬────────┘
         │ task completes, proof verifies
         ▼
   Settlement: 95% → operator pool (per contribution)
               2% → treasury
               3% → proof-verification + challenge fund
         │
         ▼
   Unused pre-auth released instantly
```

**Failed task guarantee**: If a task fails or a proof is invalid, the user pays **zero**. The cost of failure is borne by the slashed worker.

---

## 3. Operator Economics

### 3.1 Revenue Model

```
operator_revenue = task_fees + inflation_rewards − electricity − slashing_risk
```

**Worked example — RTX 4090, 24GB, 99% uptime, month:**

| Component | Value |
|-----------|-------|
| Tasks served (LLaMA-8B/70B mix) | ~45M tokens |
| Fee revenue @ avg $0.0003/1K | $13.50 |
| Inflation reward share (early network) | $85–220 |
| Electricity (450W × 0.7 duty × 24h × 30d × $0.12) | −$27.22 |
| **Net (early network)** | **$71–206/mo** |
| **Net (mature network, fees only)** | **$15–45/mo** |

> Mature-network yield depends on demand density. High-demand regions (low latency to user clusters) earn 2-3× rural averages. Stake size shifts you up the assignment priority curve (see §3.3).

### 3.2 Reward Formula (Complete)

```
R_task = R_base × M_quality × M_capability × M_uptime × M_tee

R_base      = rate(model_class) × tokens × (1 + priority_surcharge)
                priority_surcharge: LOW −10%, NORMAL 0%, HIGH +25%, CRITICAL +60%

M_quality   = 0.5 + 0.5 × (challenges_passed / challenges_issued)     [0.5–1.0]
M_capability = (log2(vram_gb) / log2(192)) × backend_efficiency       [0.35–1.0]
                backend_efficiency: CUDA 1.0, Metal 0.92, ROCm 0.95,
                                    CPU 0.55, WASM 0.35
M_uptime    = min(1.0, rolling_30d_uptime / 0.99)                     [0–1.0]
M_tee       = 1.5 if hardware TEE attested, else 0.7 (public tasks only)
```

### 3.3 Stake-Weighted Assignment

Assignment probability for a task among eligible workers:

```
P(worker) ∝ stake^0.5 × capability_score × latency_score
```

Square-root weighting prevents whale dominance: 4× stake → 2× assignment share, not 4×. This keeps the long tail of home operators viable.

### 3.4 Slashing (Full Table)

| Offense | Slash | Evidence | Appeal |
|---------|-------|----------|--------|
| Invalid ZK proof | 50% | On-chain verification failure | None (cryptographic) |
| Wrong output (challenge) | 25% | 2-of-3 redundant mismatch | Re-execution audit |
| Failed attestation | 10% | Quote verification failure | 24h re-attest window |
| Downtime > 1h | 5%/hour, cap 20% | Missing heartbeats | Automatic on recovery |
| Censorship (proven) | 15% | Multi-coordinator log audit | Governance appeal |
| Double-signing tasks | 100% + ejection | Conflicting signed receipts | None |

Slashed funds: 50% burned, 40% to challenger/treasury, 10% to reporting coordinator.

---

## 4. Delegation & Liquid Staking

### 4.1 Mechanics

```
Delegator ──stakes NEXUS──► Operator pool
                              │
                              ▼
                    Receives stNEXUS (rebasing)
                    • Transferable (DEX, collateral)
                    • Accrues operator rewards − 10% operator commission
                    • Bears pro-rata slashing risk
```

### 4.2 Risk/Return

| Role | Expected APR | Risk |
|------|-------------|------|
| Self-operator | 15-25% | Slashing, hardware costs, uptime burden |
| Delegator | 12-20% | Slashing (shared), operator choice risk |
| Treasury staker | Protocol rate | Lowest; no operator selection needed |

### 4.3 Delegator Protection

- Operator commission capped at 10% (governance-tunable)
- 7-day unbonding period (slash exposure during unbonding)
- Delegation insurance fund: 1% of treasury covers *erroneous* slashing (e.g., orchestrator bug), not operator misbehavior

---

## 5. Commercial Royalty (License Layer)

Per the **NexusLLM License v1.0**, commercial use of the software incurs a **20% royalty on Net Commercial Revenue** attributable to the software.

### 5.1 Interaction With Protocol Fees

| Usage Mode | Protocol Fee (2%) | License Royalty (20%) |
|------------|-------------------|----------------------|
| Individual, non-commercial, on public network | Yes | No |
| Business using public network API | Yes | No* |
| Business self-hosting the software | N/A | **Yes** |
| Reseller embedding NexusLLM in product | Yes | **Yes** |

*Using the *hosted public network* under standard API terms does not trigger the software license royalty — the 20% applies to *operating the software itself* commercially (forks, private deployments, white-labeling).

### 5.2 Royalty Flow

```
Commercial operator gross revenue
   − direct costs (power, hardware, bandwidth, cloud fees)
   = Net Commercial Revenue
        × 20% ──► NexusLLM treasury (licensing@nexusllm.network)

Minimum: $10,000/yr (creditable against actuals)
Payment: quarterly, USD or NEXUS at 30-day VWAP
```

### 5.3 Why This Structure

- **Non-commercial stays free** — maximizes adoption, research, and worker supply
- **Protocol users pay protocol fees** — clean separation of network vs. software value
- **Self-hosters pay royalty** — captures value from entities that bypass the network while benefiting from the codebase
- Royalty revenue funds audits, the bug bounty, and the insurance fund — directly hardening the commons

---

## 6. Treasury & Governance

### 6.1 Treasury Inflows

| Source | Est. share |
|--------|-----------|
| 2% protocol fee on inference | 55% |
| Commercial royalties (20%) | 25% |
| Slashing (40% of slashed funds) | 10% |
| Treasury staking yield | 10% |

### 6.2 Treasury Outflows (DAO-Governed)

| Category | Target |
|----------|--------|
| Security audits + bug bounties | 30% |
| Core development grants | 30% |
| Insurance fund | 15% |
| Ecosystem grants (SDKs, tooling) | 15% |
| Buyback & burn (discretionary) | 10% |

### 6.3 Governable Parameters

| Parameter | Initial | Bounds |
|-----------|---------|--------|
| Minimum stake | 10,000 NEXUS | 5,000–50,000 |
| Protocol fee | 2% | 0.5–5% |
| Slash rates | per §3.4 | ±50% relative |
| Priority surcharges | 25%/60% | 10–100% |
| Inflation decay | 1pp/yr → 2% floor | fixed schedule |
| Delegator commission cap | 10% | 5–20% |

Governance: 1 token = 1 vote, 4% quorum, 7-day voting, 48h timelock.

---

## 7. Economic Security Analysis

### 7.1 Cost of Attack

To guarantee control of all workers on a random task (defeating VRF assignment), an attacker needs ≈ 1/3 of total staked value:

```
Network at 10,000 operators × 10,000 NEXUS avg = 100M NEXUS staked
Attack cost ≈ 33M NEXUS × market price
            + hardware attestation for 3,300+ distinct machines
```

At $0.05/NEXUS that's ~$1.65M capital + 3,300 physically distinct attested machines — vastly exceeding the extractable value from serving wrong outputs (which are slashable at 50% and detectable).

### 7.2 Death-Spiral Resistance

| Shock | Dampener |
|-------|----------|
| NEXUS price crash | Stake denominated in NEXUS, but *floor price* for inference is USD-pegged → operator fiat margin compresses but doesn't invert; inflation rewards rise as % of revenue |
| Demand collapse | Operators exit → assignment share rises for remaining → per-operator yield stabilizes |
| Mass slashing event | Insurance fund covers erroneous slashes; stNEXUS rebasing prevents bank-run mechanics (7-day unbonding) |
| GPU price crash | More operators join → per-operator fees fall, but network capacity/coverage rises → attracts demand |

### 7.3 Long-Run Equilibrium

As inflation decays to 2% terminal:
- Operator revenue shifts to ~100% fee-based
- Token velocity stabilizes via payment-discount sink (5% discount for NEXUS payment)
- Treasury royalty income (20% commercial) becomes the primary security budget for audits/bounties

---

## 8. Projections (Illustrative, Not Financial Advice)

| Metric | Year 1 | Year 3 | Year 5 |
|--------|--------|--------|--------|
| Active operators | 2,000 | 25,000 | 100,000 |
| Daily inference volume | 2B tokens | 100B tokens | 1T tokens |
| Protocol fee revenue (annualized) | $0.15M | $15M | $150M |
| Operator APR (fee component) | 3% | 9% | 14% |
| Operator APR (incl. inflation) | 22% | 15% | 16% |
| Royalty revenue (commercial licensees) | $0.05M | $2M | $12M |

Assumptions: 30% demand growth/quarter Y1-Y2 decaying to 8%/quarter Y5; NEXUS price held constant (sensitivity analysis in appendix of the whitepaper).

---

## 9. Quick Reference

```bash
# Estimate earnings for your hardware
nexus operator estimate --gpu "RTX 4090" --uptime 0.99 --stake 20000

# Live network economics
nexus economics stats
# → total_stake, daily_volume, clearing_prices, apr_distribution

# Simulate reward for a task
nexus economics simulate --model llama-3-70b --tokens 50000 --priority high

# Check royalty obligations (commercial self-hosters)
nexus license check --deployment-type commercial-self-host
```

---

*Economic parameters are governance-tunable; this document reflects the genesis configuration. See `docs/TECHNICAL_WHITEPAPER.md` for formal derivations and sensitivity analysis.*
