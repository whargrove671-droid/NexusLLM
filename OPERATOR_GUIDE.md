# NexusLLM Operator Guide

## Earn by Sharing Your Compute

Turn your idle GPU/CPU into passive income. Run the NexusLLM operator, and it automatically serves inference requests from users worldwide.

---

## Quick Start

### Requirements

| Hardware | Minimum | Recommended |
|----------|---------|-------------|
| GPU VRAM | 8 GB | 24 GB+ |
| System RAM | 16 GB | 64 GB |
| Storage | 100 GB SSD | 1 TB NVMe |
| Bandwidth | 100 Mbps | 1 Gbps |
| Uptime | 90% | 99%+ |

### One-Command Start

```bash
# Docker (recommended, auto-updates)
docker run -d --name nexus-operator \
  --restart unless-stopped \
  --gpus all \
  -e NEXUS_WALLET=0xYourEthereumAddress \
  -e NEXUS_STAKE=10000 \
  -v nexus-cache:/home/nexus/cache \
  -v nexus-config:/home/nexus/config \
  ghcr.io/nexusllm/nexus-operator:latest

# Or bare metal
nexus operator start --wallet 0xYourAddress --stake 10000
```

---

## Staking & Registration

### Minimum Stake: 10,000 NEXUS (~$500)

```bash
# Check current stake requirement
nexus operator stake-info

# Stake more (increases task priority)
nexus operator stake add 5000

# Delegate to another operator (liquid staking)
nexus delegate --operator 0xOperatorAddress --amount 5000
# Receives stNEXUS (tradeable, earns yield)
```

### Wallet Setup

```bash
# Generate new wallet (if needed)
nexus wallet create

# Import existing
nexus wallet import --private-key 0x...

# Verify
nexus wallet balance
```

---

## Hardware Optimization

### NVIDIA GPUs

```bash
# Enable all features
nexus operator config set gpu.enable_cuda true
nexus operator config set gpu.enable_tensor_parallel true
nexus operator config set gpu.enable_flash_attention true

# Multi-GPU
nexus operator config set gpu.devices "0,1,2,3"

# Memory optimization
nexus operator config set gpu.memory_fraction 0.95
```

### AMD GPUs (ROCm)

```bash
nexus operator config set gpu.backend rocm
nexus operator config set gpu.enable_flash_attention true
```

### Apple Silicon

```bash
nexus operator config set gpu.backend metal
nexus operator config set gpu.mlx_enabled true
```

### CPU Only

```bash
nexus operator config set gpu.enabled false
nexus operator config set cpu.threads 16
nexus operator config set cpu.backend llama_cpp
nexus operator config set cpu.quantization Q4_K_M
```

---

## Earnings Optimization

### Maximize Your Yield

| Strategy | Impact |
|----------|--------|
| **Stake more** | 2x stake ≈ 2x task assignment |
| **High uptime** | 99%+ gets priority tasks |
| **Low latency** | <50ms RTT to coordinators |
| **TEE enabled** | 1.5x quality multiplier |
| **Latest drivers** | Avoids capability mismatch |

### Real-Time Dashboard

```bash
# Terminal dashboard
nexus operator dashboard

# Web dashboard
nexus operator dashboard --web --port 8080
# Open http://localhost:8080
```

### Key Metrics to Watch

```bash
# Earnings
nexus operator earnings --today
nexus operator earnings --month
nexus operator earnings --year

# Performance
nexus operator stats --latency
nexus operator stats --throughput
nexus operator stats --errors

# Worker health
nexus operator health
```

---

## Advanced Configuration

### Config File (`~/.config/nexus/operator.toml`)

```toml
[wallet]
address = "0x..."
keystore_path = "~/.config/nexus/keystore.json"

[stake]
amount = 50000
auto_compound = true

[hardware]
auto_detect = true
gpu_devices = "0,1"
cpu_threads = 32
memory_limit_gb = 128

[network]
listen_port = 4001
bootstrap_peers = [
  "/dns4/bootstrap1.nexusllm.network/tcp/4001/p2p/12D3KooW...",
  "/dns4/bootstrap2.nexusllm.network/tcp/4001/p2p/12D3KooW..."
]
prefer_ipv6 = false

[performance]
max_concurrent_tasks = 2
enable_tensor_parallel = true
enable_speculative = true
kv_cache_gb = 32
model_cache_gb = 200

[tee]
enabled = true
prefer_sgx = true
attestation_interval_min = 5

[earnings]
auto_withdraw_threshold = 1000  # NEXUS
withdraw_address = "0x..."
compound_rewards = true
```

### Model Preferences

```toml
[models]
# Which models to serve (empty = all compatible)
preferred_families = ["llama", "mistral", "mixtral", "phi", "codellama"]
max_model_size_b = 400
min_model_size_b = 7

# Quantization preferences (in order)
quantization_preference = ["FP16", "FP8", "INT8", "INT4", "GGUF_Q4_K_M"]

# Cache warming
warmup_on_start = true
prefetch_next_shard = true
```

---

## Monitoring & Alerts

### Prometheus Metrics

```bash
# Expose metrics
nexus operator metrics --port 9090

# Key metrics
nexus_operator_earnings_total
nexus_operator_tasks_completed_total
nexus_operator_uptime_seconds
nexus_operator_gpu_utilization_percent
nexus_operator_vram_usage_percent
nexus_operator_task_duration_seconds
nexus_operator_slashing_events_total
```

### Grafana Dashboard

Import dashboard ID: `nexusllm-operator-v1`

```bash
# Auto-install
nexus operator grafana-install --datasource prometheus
```

### Alerting Rules

```yaml
# Critical: Operator offline
- alert: OperatorOffline
  expr: nexus_operator_online == 0
  for: 2m
  labels:
    severity: critical
  annotations:
    summary: "Operator {{ $labels.instance }} offline"

# Warning: Low earnings
- alert: LowEarnings
  expr: rate(nexus_operator_earnings_total[1h]) < 10
  for: 1h
  labels:
    severity: warning

# Warning: High error rate
- alert: HighErrorRate
  expr: rate(nexus_operator_task_errors_total[5m]) > 0.05
  for: 2m
```

### Slack/Discord Notifications

```toml
[alerts]
webhook_url = "https://hooks.slack.com/services/..."
notify_on_offline = true
notify_on_slashing = true
notify_on_low_earnings = true
notify_daily_summary = true
```

---

## Maintenance

### Updates

```bash
# Auto-update (default)
nexus operator update --auto

# Manual update
nexus operator update

# Rollback
nexus operator rollback --version v1.2.3
```

### Logs

```bash
# Follow logs
nexus operator logs -f

# Filter
nexus operator logs --level error --since 1h

# Export
nexus operator logs --since 24h --format json > logs.json
```

### Health Checks

```bash
# Full health check
nexus operator health --full

# GPU health
nvidia-smi  # or rocm-smi

# Disk space
df -h /home/nexus/cache

# Network
nexus operator network-test
```

---

## Troubleshooting

### Common Issues

| Issue | Diagnosis | Fix |
|-------|-----------|-----|
| Won't register | Low stake / TEE fail | Check stake, verify TEE |
| No tasks | Capability mismatch | Update drivers, check compatibility |
| Low earnings | Low priority | Increase stake, improve uptime |
| High memory | OOM kills | Reduce `max_concurrent_tasks` |
| Slow inference | Driver/thermal | Update drivers, check temps |

### Debug Commands

```bash
# Verbose startup
nexus operator start --debug

# Capability report
nexus operator capabilities

# Network diagnostics
nexus operator network-diag

# TEE attestation test
nexus operator attestation-test
```

---

## Tax & Compliance

### Earnings Reporting

```bash
# Generate tax report
nexus operator tax-report --year 2025 --format csv

# Includes:
# - Gross earnings (NEXUS + USD)
# - Staking rewards
# - Slashing deductions
# - Gas fees paid
```

### Jurisdiction

- Operators responsible for local tax compliance
- NexusLLM provides transaction history only
- Consult tax professional for your jurisdiction

---

## Security Best Practices

### Wallet Security

- Use hardware wallet (Ledger/Trezor) for large stakes
- Never share private keys
- Use separate wallet for operator vs personal

### Server Hardening

```bash
# Firewall (only needed ports)
ufw allow 4001/tcp    # libp2p
ufw allow 9090/tcp    # Metrics (restrict to monitoring IP)
ufw deny 22/tcp       # Disable SSH if not needed

# Fail2ban for SSH
apt install fail2ban
```

### Key Rotation

```bash
# Rotate operator key annually
nexus operator rotate-key --new-key 0x...
```

---

## Scaling Up

### Multi-Node Cluster

```bash
# Coordinator node (manages workers)
nexus operator start --role coordinator --stake 100000

# Worker nodes (connect to coordinator)
nexus operator start --role worker --coordinator /ip4/1.2.3.4/tcp/4001/p2p/...
```

### Kubernetes Deployment

```yaml
# See helm/nexusllm/values.yaml for full config
workerDaemonSet:
  enabled: true
  resources:
    limits:
      nvidia.com/gpu: 1
      memory: 64Gi
```

---

## Support

- **Discord**: #operator-support
- **Email**: operators@nexusllm.network
- **Docs**: docs.nexusllm.network/operator
- **Status**: status.nexusllm.network

---

## Quick Reference

```bash
# Start
nexus operator start

# Stop
nexus operator stop

# Restart
nexus operator restart

# Status
nexus operator status

# Earnings
nexus operator earnings

# Config
nexus operator config show
nexus operator config set key value

# Update
nexus operator update

# Logs
nexus operator logs -f
```