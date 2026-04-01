# LLM Gateway (LLMProxy) – Product Requirements Document

## 1. Overview

### 1.1 Objective

Xây dựng một **LLM Gateway (LLMProxy)** self-hosted, đóng vai trò:

* API Gateway cho tất cả request LLM
* Control plane cho auth, policy, routing, cost
* Abstraction layer cho multi-provider (OpenAI, Anthropic, Gemini, local models)

### 1.2 Goals

* Chuẩn hóa API (OpenAI-compatible)
* Bảo mật cao (asymmetric signing + service account)
* Quản lý cost/budget theo tenant
* Hỗ trợ multi-provider routing + fallback
* Streaming ổn định
* Observability đầy đủ (usage, latency, error)

### 1.3 Non-goals (v1)

* Không xây training pipeline
* Không làm UI chat product
* Không làm model hosting từ đầu (chỉ integrate)

---

## 2. Personas

### 2.1 Platform Engineer

* Quản lý gateway
* Cấu hình routing, provider, policy

### 2.2 Backend Developer

* Dùng SDK để gọi LLM
* Không muốn lo auth, retry, streaming

### 2.3 Security / Ops

* Audit usage
* Theo dõi key, policy, anomaly

---

## 3. High-level Architecture

```text
Client SDK (Python)
        ↓
Gateway API (Rust)
        ↓
Core Modules
  - Auth
  - Policy Engine
  - Router
  - Streaming Engine
  - Usage Metering
        ↓
Provider Adapters
        ↓
LLM Providers / Local Models
```

---

## 4. Functional Requirements

# 4.1 API Layer (Data Plane)

### 4.1.1 Endpoints

* POST /v1/chat/completions
* POST /v1/responses
* POST /v1/embeddings
* GET /v1/models

### 4.1.2 Requirements

* OpenAI-compatible schema
* JSON + streaming support
* Validate input schema
* Normalize output schema

---

# 4.2 Authentication & Security

### 4.2.1 Auth model

* Asymmetric signing (Ed25519)
* Service account identity
* Key rotation supported

### 4.2.2 Headers

* X-Service-Account-Id
* X-Key-Id
* X-Timestamp
* X-Nonce
* X-Body-SHA256
* X-Signature

### 4.2.3 Requirements

* Verify signature
* Validate timestamp window
* Nonce replay protection
* TLS required
* Disable 0-RTT (v1)

---

# 4.3 Service Accounts

### 4.3.1 Features

* Create service account
* Attach policies
* Manage multiple keys
* Rotate/revoke keys

### 4.3.2 Requirements

* One service account = one workload identity
* Multiple keys per service account
* Audit key usage

---

# 4.4 Policy Engine

### 4.4.1 Capabilities

* Allowed models
* Max tokens
* Rate limits
* Budget limits
* Streaming permission
* Tool usage permission

### 4.4.2 Enforcement

* Before routing
* Per request

---

# 4.5 Routing Engine

### 4.5.1 Features

* Model alias → provider mapping
* Fallback chain
* Retry policy
* Timeout handling

### 4.5.2 Requirements

* Deterministic routing (v1)
* Retry on 5xx / timeout
* Fallback on provider failure

---

# 4.6 Provider Adapters

### 4.6.1 Supported providers (v1)

* OpenAI-compatible
* Anthropic

### 4.6.2 Responsibilities

* Transform request
* Normalize response
* Normalize errors
* Handle streaming

---

# 4.7 Streaming Engine

### 4.7.1 Requirements

* Support SSE / chunked stream
* Normalize events
* Handle client disconnect
* Cancel downstream request

### 4.7.2 Event types

* response.started
* response.delta
* response.completed
* response.error

---

# 4.8 Usage & Cost Metering

### 4.8.1 Track

* input tokens
* output tokens
* latency
* provider
* model
* cost

### 4.8.2 Requirements

* Per request logging
* Aggregate by tenant/service account
* Budget enforcement

---

# 4.9 Rate Limiting

### 4.9.1 Types

* per service account
* per tenant
* global

### 4.9.2 Requirements

* Sliding window
* Redis-based counter
* Reject with 429

---

# 4.10 Caching (v2)

### Scope

* embeddings
* deterministic prompts

---

# 4.11 Audit Logging

### Track

* key creation/rotation
* policy changes
* routing changes
* admin actions

---

## 5. Non-functional Requirements

### 5.1 Performance

* p95 latency < 200ms overhead
* streaming latency minimal

### 5.2 Scalability

* horizontal scaling stateless gateway
* DB connection pooling

### 5.3 Reliability

* retry + fallback
* circuit breaker (v2)

### 5.4 Security

* TLS mandatory
* no plaintext secrets
* key rotation
* replay protection

---

## 6. Data Storage

### 6.1 PostgreSQL

* service_accounts
* keys
* policies
* routes
* audit_events
* usage_events

### 6.2 Redis

* nonce cache
* rate limits
* sessions
* provider health

---

## 7. SDK Requirements (Python)

### 7.1 Features

* sync + async client
* request signing
* streaming iterator
* retry logic
* transport fallback (QUIC → HTTPS)

### 7.2 Example

```python
client = GatewayClient(...)
resp = client.responses.create(...)
```

---

## 8. Admin Console (Frontend)

### 8.1 Features

* dashboard
* request explorer
* credentials
* policies
* routing
* usage

### 8.2 Stack

* SvelteKit
* shadcn-svelte
* TypeScript

---

## 9. API Contract

### 9.1 OpenAPI

* Separate Admin API vs Data API

---

## 10. Roadmap

### Phase 1 (MVP)

* chat + responses
* auth signing
* service accounts
* routing basic
* usage logging

### Phase 2

* policies full
* retry/fallback
* dashboard
* rate limit

### Phase 3

* caching
* analytics
* advanced routing
* anomaly detection

---

## 11. Risks

* QUIC complexity
* multi-provider inconsistency
* cost miscalculation
* key management errors

---

## 12. Success Metrics

* request success rate
* latency
* cost savings
* error rate
* adoption by services

---

## 13. Open Questions

* có cần WebTransport không?
* có cần mTLS cho enterprise?
* caching strategy?
* analytics DB khi nào cần?

---

## 14. Appendix

### 14.1 Terminology

* Tenant
* Service Account
* Policy
* Provider
* Route
* Usage Event

---
