# NexusLLM User Guide

## Quick Start: Run Any Model in Seconds

### Installation

```bash
# macOS
brew install nexusllm/tap/nexus

# Linux
curl -sSL https://nexusllm.network/install.sh | sh

# Windows (PowerShell)
irm https://nexusllm.network/install.ps1 | iex

# Verify installation
nexus --version
```

### Your First Inference

```bash
# Run any model by name
nexus run llama-3-70b-instruct "Explain quantum entanglement"

# Stream tokens in real-time
nexus run mistral-7b-instruct "Write a haiku about distributed systems" --stream

# Use a local file as context
nexus run phi-3-medium "Summarize this" --file ./research_paper.pdf

# Chat mode (interactive TUI)
nexus chat
```

---

## Running Models

### Model Discovery

```bash
# List all available models
nexus models list

# Filter by capability
nexus models list --min-vram 24g --architecture llama

# See model details
nexus models info llama-3-70b-instruct
```

### Inference Options

```bash
# Basic usage
nexus run <model> "<prompt>"

# Streaming (token-by-token)
nexus run <model> "<prompt>" --stream

# File context (RAG)
nexus run <model> "<prompt>" --file ./document.pdf
nexus run <model> "<prompt>" --file ./code.py --file ./README.md

# Repository context
nexus run <model> "Review this PR" --repo github.com/user/repo#123

# Generation parameters
nexus run <model> "<prompt>" \
  --max-tokens 1024 \
  --temperature 0.7 \
  --top-p 0.95 \
  --repetition-penalty 1.1

# Speculative decoding (faster)
nexus run <model> "<prompt>" --speculative

# Priority (costs more, faster queue)
nexus run <model> "<prompt>" --priority high
```

### Chat Mode (TUI)

```bash
# Start interactive chat
nexus chat

# With specific model
nexus chat --model llama-3-70b-instruct

# With system prompt
nexus chat --system "You are a senior Rust engineer"
```

---

## Model Catalog

### Popular Models (Auto-Available)

| Model | Params | Best For | Est. Cost/1K tok |
|-------|--------|----------|------------------|
| `llama-3-8b-instruct` | 8B | General chat, coding | $0.00008 |
| `llama-3-70b-instruct` | 70B | Complex reasoning | $0.0008 |
| `llama-3-405b-instruct` | 405B | Maximum capability | $0.004 |
| `mistral-7b-instruct` | 7B | Fast general use | $0.00007 |
| `mixtral-8x7b-instruct` | 56B | Multilingual, reasoning | $0.0006 |
| `mixtral-8x22b-instruct` | 141B | Maximum MoE capability | $0.004 |
| `phi-3-mini` | 3.8B | Fast, efficient | $0.00004 |
| `phi-3-medium` | 14B | Balanced | $0.00015 |
| `codellama-7b` | 7B | Code generation | $0.00008 |
| `codellama-34b` | 34B | Complex code tasks | $0.0004 |
| `deepseek-coder-33b` | 33B | Coding specialist | $0.0004 |
| `qwen2-72b-instruct` | 72B | Multilingual | $0.0008 |
| `gemma-2-27b` | 27B | Google quality | $0.0003 |

### Custom Models

```bash
# Run any Hugging Face model
nexus run hf://meta-llama/Llama-3.1-8B-Instruct "Hello"

# Run local GGUF file
nexus run ./models/my-finetune.gguf "Test my model"

# Private model (encrypted, your workers only)
nexus run private://my-org/my-model "Internal use"
```

---

## Cost Management

### Pricing (Per 1K Tokens)

| Model Size | NexusLLM | AWS/Azure/GCP | Savings |
|------------|----------|---------------|---------|
| 7B-8B | $0.00007-0.0001 | $0.0008 | **90%** |
| 13B-14B | $0.00015-0.0002 | $0.0015 | **90%** |
| 30B-34B | $0.0003-0.0005 | $0.004 | **90%** |
| 70B-72B | $0.0007-0.001 | $0.008 | **90%** |
| 100B+ | $0.002-0.005 | $0.03-0.06 | **90%** |

### Budget Controls

```bash
# Set monthly budget
nexus config set budget.monthly 50.00

# Set per-request limit
nexus config set budget.per_request 0.50

# Cost estimation before running
nexus estimate llama-3-70b-instruct "Write a 1000 word essay"
# Estimated: ~2000 tokens = $0.0016
```

### Billing

```bash
# View usage
nexus billing usage --month 2025-01

# Export invoice
nexus billing invoice --month 2025-01 --format pdf

# Payment methods
nexus billing payment add --card
nexus billing payment add --crypto USDC
```

---

## Privacy & Security

### Your Data Never Leaves Encryption

- **TEE Execution**: All inference runs inside hardware enclaves (SGX/SEV/Secure Enclave)
- **Zero-Knowledge**: Every inference verified with ZK proofs
- **No Logs**: We never see your prompts, outputs, or model weights
- **End-to-End Encryption**: Prompt → TEE → Result, all encrypted

### Verification

```bash
# Verify last inference was in TEE
nexus verify --last

# Check ZK proof
nexus verify --proof <proof_id>

# Audit trail
nexus audit --since 2025-01-01
```

---

## Configuration

### Global Config (`~/.config/nexus/config.toml`)

```toml
# Default model
default_model = "llama-3-70b-instruct"

# Budget
[budget]
monthly = 50.00
per_request = 1.00
currency = "USD"

# Performance
[performance]
prefer_gpu = true
enable_speculative = true
max_latency_ms = 5000

# Privacy
[privacy]
require_tee = true
verify_zk_proofs = true
audit_log = true

# Network
[network]
prefer_local = true      # Prefer nearby workers
max_workers = 8          # Max parallel workers
```

### Per-Project Config (`.nexus.toml`)

```toml
# Project-specific settings
model = "codellama-34b"
budget_per_request = 0.10
system_prompt = "You are a senior Rust engineer. Write idiomatic, safe code."
context_files = ["src/**/*.rs", "Cargo.toml"]
```

---

## Advanced Usage

### Python SDK

```python
import nexus

# Simple
result = nexus.run("llama-3-70b-instruct", "Explain Rust lifetimes")

# Streaming
for token in nexus.stream("llama-3-70b-instruct", "Write a poem"):
    print(token.text, end="", flush=True)

# With context
result = nexus.run(
    model="codellama-34b",
    prompt="Add error handling",
    files=["src/main.rs"],
    max_tokens=2048,
    temperature=0.3
)

# Async
async def batch_process():
    tasks = [nexus.run_async("phi-3-mini", p) for p in prompts]
    return await asyncio.gather(*tasks)
```

### REST API

```bash
# Direct API call
curl -X POST https://api.nexusllm.network/v1/inference \
  -H "Authorization: Bearer $NEXUS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-instruct",
    "prompt": "Hello!",
    "stream": true,
    "config": {"temperature": 0.7, "max_tokens": 512}
  }'
```

```python
import requests

response = requests.post(
    "https://api.nexusllm.network/v1/inference",
    headers={"Authorization": f"Bearer {API_KEY}"},
    json={"model": "llama-3-70b", "prompt": "Hi", "stream": True},
    stream=True
)

for line in response.iter_lines():
    if line:
        print(json.loads(line.decode())["text"], end="")
```

### WebSocket (Real-time)

```javascript
const ws = new WebSocket('wss://api.nexusllm.network/v1/realtime');

ws.onopen = () => {
  ws.send(JSON.stringify({
    model: 'llama-3-70b-instruct',
    prompt: 'Tell me a story',
    stream: true
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.text) process.stdout.write(data.text);
};
```

---

## Troubleshooting

### Common Issues

| Problem | Solution |
|---------|----------|
| "Model not found" | Run `nexus models list` to see available names |
| "Insufficient workers" | Try smaller model or lower priority |
| "TEE verification failed" | Check `nexus verify --last`, retry |
| "Budget exceeded" | Increase budget or use smaller model |
| High latency | Enable `--speculative`, use `--priority high` |

### Debug Commands

```bash
# Verbose logging
nexus run llama-3-70b "test" --debug

# Show selected workers
nexus run llama-3-70b "test" --show-workers

# Dry run (cost estimate only)
nexus run llama-3-70b "test" --dry-run

# Check worker health
nexus status workers
```

---

## Getting Help

```bash
# Built-in help
nexus --help
nexus run --help
nexus chat --help

# Community
discord.gg/nexusllm
forum.nexusllm.network
github.com/nexusllm/nexusllm/issues
```

---

## Next Steps

- [Operator Guide](OPERATOR_GUIDE.md) — Earn by sharing your compute
- [API Reference](API_REFERENCE.md) — Complete API documentation
- [Architecture](ARCHITECTURE.md) — How it works under the hood
- [Security Model](SECURITY.md) — Deep dive on privacy
- [Economics](ECONOMICS.md) — Tokenomics deep dive