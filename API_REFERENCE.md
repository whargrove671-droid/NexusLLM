# NexusLLM API Reference

## Base URLs

| Environment | URL |
|-------------|-----|
| Production | `https://api.nexusllm.network/v1` |
| Staging | `https://api-staging.nexusllm.network/v1` |
| Local | `http://localhost:8080/v1` |

## Authentication

### JWT Token (Recommended)

```http
Authorization: Bearer <jwt_token>
```

- Algorithm: RS256
- Expiry: 24 hours
- Refresh: Use `/v1/auth/refresh`

### API Key (Service-to-Service)

```http
Authorization: Bearer nxs_<key_id>.<secret>
```

- Rate limit: 1000 req/min
- Scopes: `inference:read`, `inference:write`, `models:read`, `billing:read`

---

## Endpoints

### Inference

#### Submit Inference Request

```http
POST /v1/inference
Content-Type: application/json
Authorization: Bearer <token>
```

**Request Body:**

```json
{
  "model": "llama-3-70b-instruct",
  "version": "v1.2.0",
  "prompt": "Explain quantum computing",
  "stream": true,
  "config": {
    "max_tokens": 512,
    "temperature": 0.7,
    "top_p": 0.9,
    "top_k": 50,
    "repetition_penalty": 1.1,
    "stop_token_ids": [128001, 128009],
    "seed": 42,
    "speculative": {
      "enabled": true,
      "draft_model_id": "llama-3-8b-draft",
      "num_draft_tokens": 4,
      "acceptance_threshold": 0.9
    }
  },
  "priority": "NORMAL",
  "metadata": {
    "user_id": "user_123",
    "session_id": "sess_abc"
  }
}
```

**Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `model` | string | ✓ | - | Model ID from `/v1/models` |
| `version` | string | ✗ | latest | Specific model version |
| `prompt` | string | ✓* | - | Text prompt (or use `messages`) |
| `messages` | array | ✓* | - | Chat format (alternative to `prompt`) |
| `stream` | boolean | ✗ | false | Stream tokens via SSE |
| `config` | object | ✗ | {} | Generation config |
| `priority` | enum | ✗ | NORMAL | LOW, NORMAL, HIGH, CRITICAL |
| `metadata` | object | ✗ | {} | Custom metadata |

*Either `prompt` or `messages` required.

**Generation Config:**

| Field | Type | Default | Range |
|-------|------|---------|-------|
| `max_tokens` | int | 512 | 1-8192 |
| `temperature` | float | 0.7 | 0.0-2.0 |
| `top_p` | float | 0.9 | 0.0-1.0 |
| `top_k` | int | 50 | 0-100 |
| `repetition_penalty` | float | 1.1 | 1.0-2.0 |
| `stop_token_ids` | array | [] | - |
| `seed` | int | random | - |
| `speculative` | object | null | See below |

**Speculative Config:**

| Field | Type | Default |
|-------|------|---------|
| `enabled` | bool | false |
| `draft_model_id` | string | auto |
| `num_draft_tokens` | int | 4 |
| `acceptance_threshold` | float | 0.9 |

**Response (Non-Streaming):**

```json
{
  "request_id": "req_abc123",
  "task_id": "task_xyz789",
  "status": "COMPLETED",
  "output": "Quantum computing uses quantum bits...",
  "stats": {
    "prompt_tokens": 15,
    "completion_tokens": 256,
    "ttft_ms": 145.2,
    "tpot_ms": 32.1,
    "total_time_ms": 8350.5,
    "cost_usd": 0.0002048
  },
  "workers": [
    {"peer_id": "12D3KooW...", "shard": "layers_0_15", "duration_ms": 1200}
  ],
  "zk_proof": "0x...",
  "tee_attestation": "0x..."
}
```

**Response (Streaming - SSE):**

```
data: {"request_id": "req_abc123", "token_id": 1234, "text": "Quantum", "logprob": -0.12, "is_final": false}
data: {"request_id": "req_abc123", "token_id": 5678, "text": " computing", "logprob": -0.08, "is_final": false}
...
data: {"request_id": "req_abc123", "token_id": 0, "text": "", "logprob": 0, "is_final": true, "stats": {...}}
data: [DONE]
```

#### Chat Completion (OpenAI Compatible)

```http
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer <token>
```

```json
{
  "model": "llama-3-70b-instruct",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant"},
    {"role": "user", "content": "Hello!"}
  ],
  "stream": true,
  "temperature": 0.7,
  "max_tokens": 512
}
```

---

### Models

#### List Models

```http
GET /v1/models?architecture=llama&min_params=1000000000&max_params=100000000000
```

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `architecture` | string | Filter by architecture (llama, mistral, mixtral, etc.) |
| `min_params` | int | Minimum parameter count |
| `max_params` | int | Maximum parameter count |
| `quantization` | string | Filter by quantization (FP16, INT4, etc.) |
| `page` | int | Page number (default: 1) |
| `page_size` | int | Items per page (default: 20, max: 100) |

**Response:**

```json
{
  "models": [
    {
      "model_id": "llama-3-70b-instruct",
      "latest_version": "v1.2.0",
      "architecture": "llama",
      "total_params": 70000000000,
      "available_quants": ["FP16", "FP8", "INT8", "INT4", "GGUF_Q4_K_M"],
      "context_length": 8192,
      "estimated_cost_per_1k_tokens_usd": 0.0008,
      "is_active": true,
      "tags": ["chat", "coding", "reasoning"]
    }
  ],
  "total": 42,
  "page": 1,
  "page_size": 20
}
```

#### Get Model Details

```http
GET /v1/models/{model_id}
```

**Response:**

```json
{
  "model_id": "llama-3-70b-instruct",
  "versions": [
    {
      "version": "v1.2.0",
      "manifest_hash": "sha256:abc...",
      "shards": [
        {"shard_id": "layers_0_15", "layers": [0, 15], "size_gb": 14, "min_vram_gb": 16}
      ],
      "pipeline_stages": 4,
      "released_at": "2025-01-15T10:00:00Z"
    }
  ],
  "benchmark_scores": {
    "mmlu": 0.82,
    "gsm8k": 0.92,
    "humaneval": 0.81
  }
}
```

---

### Task Management

#### Get Task Status

```http
GET /v1/tasks/{task_id}
```

**Response:**

```json
{
  "task_id": "task_xyz789",
  "request_id": "req_abc123",
  "status": "RUNNING",
  "progress": 0.65,
  "current_stage": 2,
  "total_stages": 4,
  "workers": [
    {"peer_id": "12D3KooW...", "shard": "layers_0_15", "status": "COMPLETED"},
    {"peer_id": "12D3KooW...", "shard": "layers_16_31", "status": "RUNNING"}
  ],
  "estimated_completion_ms": 3200
}
```

#### Cancel Task

```http
DELETE /v1/tasks/{task_id}
```

---

### WebSocket (Real-time)

```http
GET /v1/realtime
Upgrade: websocket
Authorization: Bearer <token>
```

**Messages:**

```json
// Client → Server
{"type": "inference", "model": "llama-3-70b", "prompt": "Hi", "stream": true}

// Server → Client (token)
{"type": "token", "request_id": "req_123", "token_id": 1234, "text": "Hello", "logprob": -0.1, "is_final": false}

// Server → Client (done)
{"type": "done", "request_id": "req_123", "stats": {...}}

// Server → Client (error)
{"type": "error", "request_id": "req_123", "code": "MODEL_NOT_FOUND", "message": "..."}
```

---

### Billing

#### Get Usage

```http
GET /v1/billing/usage?month=2025-01
```

**Response:**

```json
{
  "month": "2025-01",
  "total_requests": 1247,
  "total_tokens": 45230000,
  "total_cost_usd": 23.45,
  "by_model": [
    {"model": "llama-3-70b", "requests": 500, "tokens": 25000000, "cost_usd": 20.00},
    {"model": "phi-3-mini", "requests": 747, "tokens": 20230000, "cost_usd": 3.45}
  ]
}
```

#### Get Invoice

```http
GET /v1/billing/invoice?month=2025-01&format=pdf
```

---

### Health

```http
GET /health
```

```json
{
  "status": "healthy",
  "version": "1.2.3",
  "uptime_seconds": 86400,
  "workers_online": 1247,
  "queue_depth": 23
}
```

---

## Error Codes

| Code | HTTP | Description |
|------|------|-------------|
| `MODEL_NOT_FOUND` | 404 | Model ID not in registry |
| `VERSION_NOT_FOUND` | 404 | Specific version unavailable |
| `INSUFFICIENT_WORKERS` | 503 | No eligible workers online |
| `BUDGET_EXCEEDED` | 402 | Request exceeds budget limit |
| `INVALID_CONFIG` | 400 | Generation config validation failed |
| `TEE_VERIFICATION_FAILED` | 500 | Worker TEE attestation invalid |
| `ZK_PROOF_INVALID` | 500 | Zero-knowledge proof verification failed |
| `RATE_LIMITED` | 429 | Too many requests |
| `UNAUTHORIZED` | 401 | Invalid or expired token |
| `INTERNAL_ERROR` | 500 | Unexpected server error |

---

## Rate Limits

| Tier | Requests/min | Tokens/min | Concurrent |
|------|--------------|------------|------------|
| Free | 30 | 50,000 | 2 |
| Pro | 120 | 500,000 | 8 |
| Enterprise | 1000 | 10,000,000 | 50 |

---

## SDKs

| Language | Package | Install |
|----------|---------|---------|
| Python | `nexusllm` | `pip install nexusllm` |
| TypeScript | `@nexusllm/sdk` | `npm install @nexusllm/sdk` |
| Go | `github.com/nexusllm/go-sdk` | `go get github.com/nexusllm/go-sdk` |
| Rust | `nexus-sdk` | `cargo add nexus-sdk` |

---

## Examples

### Python

```python
import nexus

# Sync
client = nexus.Client(api_key="nxs_...")
result = client.inference(
    model="llama-3-70b-instruct",
    prompt="Explain Rust ownership",
    max_tokens=1024,
    temperature=0.3
)

# Async streaming
async for token in client.stream("llama-3-70b", "Write a poem"):
    print(token.text, end="", flush=True)
```

### TypeScript

```typescript
import { NexusClient } from '@nexusllm/sdk';

const client = new NexusClient({ apiKey: 'nxs_...' });

// Streaming
for await (const token of client.stream('llama-3-70b', 'Hello')) {
  process.stdout.write(token.text);
}
```

### cURL

```bash
curl -X POST https://api.nexusllm.network/v1/inference \
  -H "Authorization: Bearer nxs_..." \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3-70b", "prompt": "Hi", "stream": true}'
```

---

## Versioning

- API version in URL: `/v1/`
- Model versions: Semantic (v1.2.0)
- Breaking changes: New version path (`/v2/`)
- Deprecation notice: 90 days advance

---

## Support

- API Status: https://status.nexusllm.network
- Docs: https://docs.nexusllm.network/api
- Contact: api-support@nexusllm.network