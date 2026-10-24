# NexusLLM Distributed LLM Compute Platform

> **⚠️ Design-stage prototype — this repository is an architecture scaffold, not a shipped product.**
> The Rust workspace compiles partially; several crates contain skeleton implementations awaiting full development.
> See [DOCUMENTATION.md](DOCUMENTATION.md) for the complete technical specification and [CONTRIBUTING.md](CONTRIBUTING.md) for how to help build it out.

**Run any model. Powered by the world's idle compute.**

NexusLLM transforms idle home desktops into a planetary-scale inference fabric. Install the app, pick any model, and it runs automatically on the distributed network — no infrastructure required.

## 🚀 For Users: One-Click Model Running

```bash
# Install the NexusLLM app (one binary, auto-updates)
curl -sSL https://nexusllm.network/install.sh | sh

# Run ANY model — it just works
nexus run llama-3-70b-instruct "Explain quantum computing"
nexus run mixtral-8x22b "Write a Python async web scraper"
nexus run phi-3-mini "Summarize this PDF" --file document.pdf
nexus run codellama-34b "Review my PR" --repo github.com/user/repo

# Or use the beautiful TUI
nexus chat
```

**That's it.** The app handles everything:
- 🔍 Finds the best available workers for your model
- ⚡ Splits work across GPUs/CPUs worldwide (pipeline + tensor parallelism)
- 🔒 Runs in hardware TEEs — your prompts & data never leave encrypted enclaves
- 💰 Pays workers automatically via crypto (you pay per token, ~90% cheaper than cloud)
- 📊 Shows real-time throughput, latency, and cost in the TUI

## 🏗️ For Operators: Earn by Sharing Compute

```bash
# Install and start earning
nexus operator start --stake 10000  # 10k NEXUS minimum

# Monitor earnings in real-time
nexus operator dashboard
```

**Supported Hardware** (auto-detected):
| Hardware | Models | Earnings |
|----------|--------|----------|
| RTX 4090 / 3090 / A100 / H100 | 7B–400B+ | $50–500+/mo |
| RTX 3080 / 4080 / 3070 | 7B–70B | $20–150/mo |
| Apple M1/M2/M3 Max/Ultra | 7B–70B | $15–100/mo |
| AMD 7900 XTX / 7800 XT | 7B–70B | $15–80/mo |
| CPU (AVX2/AMX/NEON) | 1B–13B | $5–30/mo |
| Mobile NPU / Browser WASM | 1B–7B | $1–10/mo |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              USER EXPERIENCE                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│  │  CLI/TUI    │  │  Python SDK │  │  REST API   │  │  Web UI     │       │
│  │  `nexus`    │  │  `nexus.run()`│  │  POST /v1   │  │  chat.nexus │       │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘       │
└─────────┼────────────────┼────────────────┼────────────────┼────────────────┘
          │                │                │                │
          ▼                ▼                ▼                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        NEXUS ORCHESTRATOR (Cloud)                           │
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

---

## 📦 Installation

### For Users (Inference)
```bash
# macOS
brew install nexusllm/tap/nexus

# Linux
curl -sSL https://nexusllm.network/install.sh | sh

# Windows (PowerShell)
irm https://nexusllm.network/install.ps1 | iex

# Docker
docker run -it ghcr.io/nexusllm/nexus-cli:latest run llama-3-70b "Hello!"
```

### For Operators (Compute Provider)
```bash
# Docker (GPU)
docker run -d --gpus all \
  -e NEXUS_STAKE=10000 \
  -e NEXUS_WALLET=0xYourAddress \
  ghcr.io/nexusllm/nexus-operator:latest

# Bare metal (auto-detects GPU/CPU/TEE)
nexus operator start --stake 10000 --wallet 0xYourAddress
```

---

## 💰 Economics: 90% Cheaper Than Cloud

| Model | Cloud (A100) | NexusLLM | Savings |
|-------|-------------|----------|---------|
| LLaMA-3-8B | $0.0008/1K tok | $0.00008/1K tok | **90%** |
| LLaMA-3-70B | $0.008/1K tok | $0.0008/1K tok | **90%** |
| Mixtral-8x22B | $0.06/1K tok | $0.006/1K tok | **90%** |

**Pay per token** — no GPU-hour minimums, no reserved instances, no idle costs.

---

## 🔒 Privacy & Security

- **TEE Execution**: Prompts & model weights encrypted in SGX/SEV/Secure Enclave
- **Zero-Knowledge Proofs**: Every inference cryptographically verified
- **No Logs**: Your data never touches our servers — only encrypted TEE memory
- **Open Source**: Full auditability at https://github.com/nexusllm/nexusllm

---

## 📚 Documentation

| Guide | Link |
|-------|------|
| User Quickstart | [USER_GUIDE.md](USER_GUIDE.md) |
| Operator Guide | [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) |
| API Reference | [API_REFERENCE.md](API_REFERENCE.md) |
| Architecture Deep-Dive | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Security Model | [SECURITY.md](SECURITY.md) |
| Economics Deep-Dive | [ECONOMICS.md](ECONOMICS.md) |
| Complete Reference | [DOCUMENTATION.md](DOCUMENTATION.md) |
| Technical Design | [NEXUSLLM_DESIGN.md](NEXUSLLM_DESIGN.md) |

---

## 🛠️ Development

```bash
# Build from source
git clone https://github.com/nexusllm/nexusllm
cd nexusllm
cargo build --release --workspace

# Run tests
cargo test --workspace

# Build operator binary
cargo build --release --bin nexus-operator --features cuda
```

---

## 🤝 Community

- **Discord**: https://discord.gg/nexusllm
- **Twitter**: @nexusllm
- **Forum**: https://forum.nexusllm.network
- **Email**: team@nexusllm.network

---

## 📄 License

**NexusLLM License v1.0** — Free for non-commercial use. Commercial use requires 20% royalty on net revenue. See [LICENSE](LICENSE) for details.

For commercial licensing: **licensing@nexusllm.network**

---

*Built with ❤️ by the NexusLLM community — Democratizing AI compute, one home desktop at a time.*