# LLMSmartGate -- High-Level Technical Solution Design (HLD)

> **Doc ID:** `HLD-001`  
> **Version:** 1.0  
> **Date:** 2026-04-01  
> **Status:** Approved  
> **Classification:** Internal -- Engineering

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Solution Architect |
| **Perspective** | System decomposition, data flows, infrastructure, integration |
| **Primary Audience** | Tech Lead, Rust Developer, Svelte Developer, DevOps |
| **Secondary Audience** | Product Owner (for scope validation), Security Team |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| `[C4-Lx]` | C4 model diagram level (L1 = Context, L2 = Container, L3 = Component) |
| `───►` | Synchronous call / data flow direction |
| `- - ►` | Asynchronous / event-driven flow |
| `[v1]` / `[v2]` | Feature available in Phase 1 (MVP) vs Phase 2+ |
| `⚠` | Security-sensitive boundary or decision |

### Reading Order Suggestion

1. **C4 Diagrams (Sections 1-3)** -- Zoom from 10,000ft → component level
2. **Data Flow Diagrams (Section 4)** -- Understand request lifecycle
3. **Security Architecture (Section 7)** -- Threat model & auth flows
4. **Infrastructure (Section 5)** -- Deployment & network design
5. **Reliability + Scalability (Sections 8-9)** -- Failure handling & growth

### Document Relationships

```
PRD-001                -- Requirements driving this design
  ├── ARC-001          -- Standards this design must comply with
  ├── HLD-001 (this)   -- Big picture: how components interact
  ├── LLD-BE-001       -- Drills into each HLD component (Rust)
  ├── LLD-FE-001       -- Drills into Admin Console component
  └── TST-001          -- Validates HLD data flows & integration points
```

---

## Table of Contents

1. [System Context Diagram (C4 Level 1)](#1-system-context-diagram-c4-level-1)
2. [Container Diagram (C4 Level 2)](#2-container-diagram-c4-level-2)
3. [Component Diagram (C4 Level 3)](#3-component-diagram-c4-level-3---gateway-service)
4. [Data Flow Diagrams](#4-data-flow-diagrams)
5. [Infrastructure Architecture](#5-infrastructure-architecture)
6. [Integration Architecture](#6-integration-architecture)
7. [Security Architecture](#7-security-architecture)
8. [Scalability Architecture](#8-scalability-architecture)
9. [Reliability Architecture](#9-reliability-architecture)
10. [Observability Architecture](#10-observability-architecture)

---

## 1. System Context Diagram (C4 Level 1)

This diagram shows LLMSmartGate and its relationships with external actors and systems.

```
 +-------------------+      +--------------------+      +---------------------+
 |                   |      |                    |      |                     |
 | Backend Developer |      | Platform Engineer  |      |   Security / Ops    |
 |   (Person)        |      |    (Person)        |      |     (Person)        |
 |                   |      |                    |      |                     |
 +--------+----------+      +--------+-----------+      +---------+-----------+
          |                          |                            |
          | Uses Python SDK          | Uses Admin Console         | Reviews Audit Logs
          | to call LLM APIs         | to manage gateway          | and Dashboards
          |                          |                            |
          v                          v                            v
 +--------+----------+      +--------+-----------+      +---------+-----------+
 |                   |      |                    |      |                     |
 |  Client App       |      |  Admin Console     |      |  Monitoring Stack   |
 |  (Python SDK)     |      |  (SvelteKit SPA)   |      |  (Grafana/Prometheus|
 |                   |      |                    |      |   Tempo/Loki)       |
 +--------+----------+      +--------+-----------+      +---------+-----------+
          |                          |                            ^
          | HTTPS (Ed25519 signed)   | HTTPS (JWT auth)           | OTLP
          |                          |                            |
          v                          v                            |
 +--------+--------------------------------------------------------------------------+
 |                                                                                    |
 |                         LLMSmartGate Gateway                                       |
 |                        [Software System]                                           |
 |                                                                                    |
 |  Self-hosted LLM API gateway providing unified OpenAI-compatible endpoints         |
 |  with multi-tenant auth, policy enforcement, provider routing, usage metering,     |
 |  and full observability.                                                           |
 |                                                                                    |
 +---------+----------+----------+----------+-----------+----------+------------------+
           |          |          |          |           |          |
           |          |          |          |           |          |
           v          v          v          v           v          v
 +---------+-+ +------+---+ +---+------+ +-+--------+ ++---------++ +--------+
 |           | |          | |          | |          | |           | |        |
 |  OpenAI   | | Anthropic| | Google   | |  Azure   | |   vLLM   | |  More  |
 |  API      | |  API     | | Gemini   | | OpenAI   | | (Local)  | |  ...   |
 |           | |          | |  API     | |  API     | |          | |        |
 +-----------+ +----------+ +----------+ +----------+ +----------+ +--------+
    [External Systems -- LLM Providers]
```

**Key relationships:**

| From | To | Protocol | Description |
|---|---|---|---|
| Client App (SDK) | Gateway | HTTPS | Ed25519-signed API requests (data plane) |
| Admin Console | Gateway | HTTPS | JWT-authenticated admin operations (control plane) |
| Gateway | LLM Providers | HTTPS | Transformed LLM API calls |
| Gateway | Monitoring Stack | OTLP/gRPC | Traces, metrics, and logs export |

---

## 2. Container Diagram (C4 Level 2)

This diagram shows the internal containers (deployable units) within LLMSmartGate.

```
+-----------------------------------------------------------------------------------+
|                              LLMSmartGate System                                  |
|                                                                                   |
|  +------------------+          +------------------+                               |
|  |                  | HTTPS/   |                  |                               |
|  |  Admin Console   +--------->+  Gateway API     |                               |
|  |  [Container]     | JWT Auth |  [Container]     |                               |
|  |                  |          |                  |                               |
|  |  SvelteKit 2     |          |  Rust / Axum     |                               |
|  |  Svelte 5        |          |  Async runtime   |                               |
|  |  shadcn-svelte   |          |  Tower middleware |                               |
|  |  TypeScript      |          |                  |                               |
|  +------------------+          +---+---------+----+                               |
|                                    |         |                                    |
|             +----------------------+         +---------------------+              |
|             |                                                      |              |
|             v                                                      v              |
|  +----------+---------+                              +-------------+----------+   |
|  |                    |                              |                        |   |
|  |  PostgreSQL        |                              |  Redis                 |   |
|  |  [Container: DB]   |                              |  [Container: Cache]    |   |
|  |                    |                              |                        |   |
|  |  - tenants         |                              |  - nonce replay cache  |   |
|  |  - service_accounts|                              |  - rate limit counters |   |
|  |  - keys            |                              |  - admin sessions      |   |
|  |  - policies        |                              |  - provider health     |   |
|  |  - routes          |                              |  - config cache        |   |
|  |  - usage_events    |                              |                        |   |
|  |  - audit_events    |                              +------------------------+   |
|  |                    |                                                           |
|  +--------------------+                                                           |
+-----------------------------------------------------------------------------------+
         ^                                    ^
         | Python SDK                         | OTLP/gRPC
         | (HTTPS, Ed25519)                   |
+--------+----------+              +----------+-----------+
|                   |              |                      |
|  Client App       |              |  OTel Collector      |
|  [External]       |              |  [Container]         |
|                   |              |                      |
+-------------------+              |  +--> Tempo (traces) |
                                   |  +--> Prometheus     |
                                   |  |    (metrics)      |
                                   |  +--> Loki (logs)    |
                                   +----------------------+
```

**Container responsibilities:**

| Container | Technology | Responsibility |
|---|---|---|
| Gateway API | Rust, Axum, Tokio | Data plane + Admin API, auth, policy, routing, streaming, usage metering |
| Admin Console | SvelteKit 2, Svelte 5, shadcn-svelte | Web UI for tenant/account/policy/route management, usage dashboards |
| PostgreSQL | PostgreSQL 16 | Persistent storage for control plane config and usage/audit events |
| Redis | Redis 7 (Cluster or Sentinel) | Ephemeral storage for nonce cache, rate limits, sessions, health probes |
| OTel Collector | OpenTelemetry Collector | Telemetry pipeline: receive OTLP, export to backends |

---

## 3. Component Diagram (C4 Level 3 -- Gateway Service)

This diagram shows the internal components within the Gateway API container.

```
+---------------------------------------------------------------------------------+
|                            Gateway API Container                                |
|                            (Rust / Axum / Tokio)                                |
|                                                                                 |
|  +----------------------------+     +----------------------------+              |
|  |     Data Plane API         |     |     Admin Plane API        |              |
|  |     [Component]            |     |     [Component]            |              |
|  |                            |     |                            |              |
|  |  /v1/chat/completions      |     |  /admin/v1/tenants         |              |
|  |  /v1/responses             |     |  /admin/v1/service-accounts|              |
|  |  /v1/embeddings            |     |  /admin/v1/keys            |              |
|  |  /v1/models                |     |  /admin/v1/policies        |              |
|  +-------------+--------------+     |  /admin/v1/routes          |              |
|                |                    |  /admin/v1/usage           |              |
|                v                    |  /admin/v1/audit           |              |
|  +-------------+--------------+     +-------------+--------------+              |
|  |     Auth Module            |                   |                             |
|  |     [Component]            |                   v                             |
|  |                            |     +-------------+--------------+              |
|  |  - Signature verification  |     |     JWT Auth Module        |              |
|  |  - Nonce replay check      |     |     [Component]            |              |
|  |  - Timestamp validation    |     |                            |              |
|  |  - Key lookup              |     |  - Token verification      |              |
|  +-------------+--------------+     |  - Role-based access       |              |
|                |                    +----------------------------+              |
|                v                                                                |
|  +-------------+--------------+                                                 |
|  |     Policy Engine          |                                                 |
|  |     [Component]            |                                                 |
|  |                            |                                                 |
|  |  - Model access control    |                                                 |
|  |  - Token limits            |                                                 |
|  |  - Rate limit check        |                                                 |
|  |  - Budget enforcement      |                                                 |
|  |  - Feature flags           |                                                 |
|  +-------------+--------------+                                                 |
|                |                                                                |
|                v                                                                |
|  +-------------+--------------+                                                 |
|  |     Routing Engine         |                                                 |
|  |     [Component]            |                                                 |
|  |                            |                                                 |
|  |  - Model alias resolution  |                                                 |
|  |  - Priority-based routing  |                                                 |
|  |  - Fallback chain          |                                                 |
|  |  - Provider health check   |                                                 |
|  +-------------+--------------+                                                 |
|                |                                                                |
|                v                                                                |
|  +-------------+--------------+     +----------------------------+              |
|  |     Provider Adapters      |     |     Streaming Engine       |              |
|  |     [Component]            |     |     [Component]            |              |
|  |                            |     |                            |              |
|  |  - OpenAI adapter          |     |  - SSE relay               |              |
|  |  - Anthropic adapter       |<--->|  - Event normalization     |              |
|  |  - Gemini adapter          |     |  - Disconnect detection    |              |
|  |  - Azure OpenAI adapter    |     |  - Upstream cancellation   |              |
|  |  - vLLM adapter            |     |                            |              |
|  +-------------+--------------+     +----------------------------+              |
|                |                                                                |
|                v                                                                |
|  +-------------+--------------+     +----------------------------+              |
|  |     Usage Metering         |     |     Audit Logger           |              |
|  |     [Component]            |     |     [Component]            |              |
|  |                            |     |                            |              |
|  |  - Token counting          |     |  - Admin action logging    |              |
|  |  - Cost estimation         |     |  - Security event capture  |              |
|  |  - Async persistence       |     |  - Immutable audit trail   |              |
|  +----------------------------+     +----------------------------+              |
|                                                                                 |
|  +----------------------------+                                                 |
|  |     Observability          |                                                 |
|  |     [Component]            |                                                 |
|  |                            |                                                 |
|  |  - Structured logging      |                                                 |
|  |  - Distributed tracing     |                                                 |
|  |  - Prometheus metrics       |                                                 |
|  +----------------------------+                                                 |
+---------------------------------------------------------------------------------+
         |                    |                    |
         v                    v                    v
    PostgreSQL             Redis              OTel Collector
```

**Component interaction summary:**

| Source Component | Target Component | Interaction |
|---|---|---|
| Data Plane API | Auth Module | Verify every data plane request |
| Auth Module | Redis | Check nonce uniqueness |
| Auth Module | PostgreSQL | Fetch public key for service account |
| Policy Engine | Redis | Check rate limits, budget snapshot |
| Policy Engine | PostgreSQL | Load policy definition |
| Routing Engine | Redis | Read provider health cache |
| Provider Adapters | External LLM APIs | HTTP requests to upstream providers |
| Provider Adapters | Streaming Engine | Delegate SSE relay for streaming requests |
| Usage Metering | PostgreSQL | Async insert usage_events |
| Audit Logger | PostgreSQL | Insert audit_events |
| Observability | OTel Collector | Export traces, metrics, logs |

---

## 4. Data Flow Diagrams

### 4.1 Non-Streaming Request Flow

```
Client (SDK)                 Gateway                          Provider (e.g., OpenAI)
    |                           |                                      |
    |  POST /v1/chat/completions|                                      |
    |  Headers: X-Signature,    |                                      |
    |  X-Timestamp, X-Nonce,    |                                      |
    |  X-Key-Id, etc.           |                                      |
    |  Body: {model, messages}  |                                      |
    |-------------------------->|                                      |
    |                           |                                      |
    |                    [1. Parse & Validate JSON]                     |
    |                    [2. Generate request_id]                       |
    |                    [3. Start trace span]                         |
    |                           |                                      |
    |                    [4. AUTH]                                      |
    |                    | - Reconstruct canonical string               |
    |                    | - Fetch public key (PG)                      |
    |                    | - Verify Ed25519 signature                   |
    |                    | - Validate timestamp (+/- 300s)              |
    |                    | - Check nonce uniqueness (Redis SET NX EX)   |
    |                    | - Resolve tenant + service_account context   |
    |                           |                                      |
    |                    [5. POLICY]                                    |
    |                    | - Load policy for service_account            |
    |                    | - Check model access                         |
    |                    | - Check token limits                         |
    |                    | - Check rate limit (Redis INCR + EXPIRE)     |
    |                    | - Check budget remaining                     |
    |                           |                                      |
    |                    [6. ROUTING]                                   |
    |                    | - Resolve model alias to provider route      |
    |                    | - Check provider health (Redis)              |
    |                    | - Build attempt plan [primary, fallback...]  |
    |                           |                                      |
    |                    [7. PROVIDER CALL (attempt 1)]                 |
    |                    | - Transform to provider-specific format      |
    |                    | - Inject provider auth (API key)             |
    |                    | - Inject traceparent header                  |
    |                           |                                      |
    |                           |  POST https://api.openai.com/v1/...  |
    |                           |---------------------------------------->|
    |                           |                                      |
    |                           |          200 OK + response body      |
    |                           |<----------------------------------------|
    |                           |                                      |
    |                    [8. Transform response to OpenAI format]       |
    |                    [9. USAGE METERING (async)]                    |
    |                    | - Count tokens                               |
    |                    | - Calculate cost                              |
    |                    | - Spawn task: INSERT usage_events (PG)       |
    |                    | - Update budget counter (Redis)              |
    |                           |                                      |
    |  200 OK                   |                                      |
    |  {id, choices, usage}     |                                      |
    |<--------------------------|                                      |
    |                           |                                      |
```

**Latency breakdown (non-streaming, P95):**

| Phase | Budget |
|---|---|
| Parse + validate | 1 ms |
| Auth | 4 ms |
| Policy | 2 ms |
| Routing | 0.5 ms |
| Request transform | 1 ms |
| Provider call | 200ms - 60s (variable) |
| Response transform | 1 ms |
| Usage metering | 0 ms (async) |
| **Total gateway overhead** | **~9.5 ms** |

### 4.2 Streaming Request Flow

```
Client (SDK)                 Gateway                          Provider
    |                           |                                |
    |  POST /v1/chat/completions|                                |
    |  Body: {stream: true, ...}|                                |
    |-------------------------->|                                |
    |                           |                                |
    |                    [1-6. Same as non-streaming]             |
    |                           |                                |
    |                    [7. PROVIDER CALL (streaming)]           |
    |                           |  POST (stream: true)           |
    |                           |------------------------------->|
    |                           |                                |
    |  HTTP 200                 |                                |
    |  Content-Type: text/      |                                |
    |  event-stream             |                                |
    |<--------------------------|                                |
    |                           |                                |
    |                    [8. STREAMING ENGINE starts relay]       |
    |                           |                                |
    |                           |  data: {"choices":[...]}       |
    |                           |<-------------------------------|
    |                    [Normalize to gateway event format]      |
    |  data: {"choices":[...]}  |                                |
    |<--------------------------|                                |
    |                           |                                |
    |                           |  data: {"choices":[...]}       |
    |                           |<-------------------------------|
    |  data: {"choices":[...]}  |                                |
    |<--------------------------|                                |
    |                           |                                |
    |        ...relay continues chunk by chunk...                |
    |                           |                                |
    |                           |  data: [DONE]                  |
    |                           |<-------------------------------|
    |                    [9. Finalize stream]                     |
    |                    | - Accumulate total tokens              |
    |                    | - Calculate cost                       |
    |                    | - Async: INSERT usage_events           |
    |  data: [DONE]             |                                |
    |<--------------------------|                                |
    |                           |                                |

--- CLIENT DISCONNECT SCENARIO ---

    |  [Client closes connection]
    |-------X                   |                                |
    |                    [Detect disconnect via tokio::select!]   |
    |                    [Send cancellation to provider]          |
    |                           |  [Abort/drop request]          |
    |                           |------------------------------->|
    |                    [Mark usage as partial]                  |
    |                    [Persist partial usage_event]            |
```

**Streaming design decisions:**

- **Zero-copy relay:** SSE chunks from the provider are parsed for event normalization but the payload bytes are forwarded using `bytes::Bytes` without allocation.
- **Disconnect detection:** `tokio::select!` races the provider stream against a client connection liveness check. On disconnect, the upstream reqwest request is dropped, which cancels the underlying TCP stream.
- **Token accumulation:** Token counts from streaming are accumulated from each chunk's delta. The final `[DONE]` event may include total usage; if not, the gateway uses accumulated counts.

### 4.3 Admin Operation Flow

```
Platform Engineer            Admin Console             Gateway (Admin API)     PostgreSQL
      |                           |                           |                    |
      | Open browser, navigate    |                           |                    |
      |-------------------------->|                           |                    |
      |                           |                           |                    |
      | Login (username/password) |                           |                    |
      |-------------------------->|                           |                    |
      |                           | POST /admin/v1/auth/login |                    |
      |                           |-------------------------->|                    |
      |                           |                           | Verify credentials |
      |                           |                           |------------------>|
      |                           |                           |<------------------|
      |                           |                           | Issue JWT         |
      |                           | 200 {access_token, ...}   | (short-lived)     |
      |                           |<--------------------------|                    |
      |                           |                           |                    |
      | Create service account    |                           |                    |
      |-------------------------->|                           |                    |
      |                           | POST /admin/v1/           |                    |
      |                           |   service-accounts        |                    |
      |                           | Authorization: Bearer JWT |                    |
      |                           |-------------------------->|                    |
      |                           |                    [1. Validate JWT]            |
      |                           |                    [2. Check RBAC role]         |
      |                           |                    [3. Validate payload]        |
      |                           |                           |                    |
      |                           |                           | INSERT INTO        |
      |                           |                           | service_accounts   |
      |                           |                           |------------------>|
      |                           |                           |<------------------|
      |                           |                           |                    |
      |                           |                           | INSERT INTO        |
      |                           |                           | audit_events       |
      |                           |                           |------------------>|
      |                           |                           |<------------------|
      |                           |                           |                    |
      |                           | 201 {service_account}     |                    |
      |                           |<--------------------------|                    |
      | Display result            |                           |                    |
      |<--------------------------|                           |                    |
```

### 4.4 Key Rotation Flow

```
Platform Engineer    Admin Console       Gateway (Admin API)    PostgreSQL         Redis
      |                   |                    |                    |                |
      | Initiate key      |                    |                    |                |
      | rotation for SA   |                    |                    |                |
      |------------------>|                    |                    |                |
      |                   |                    |                    |                |
      | Step 1: Register  |                    |                    |                |
      | new public key    |                    |                    |                |
      |------------------>|                    |                    |                |
      |                   | POST /admin/v1/    |                    |                |
      |                   | service-accounts/  |                    |                |
      |                   | {id}/keys          |                    |                |
      |                   | {public_key: "..."} |                   |                |
      |                   |------------------->|                    |                |
      |                   |                    | INSERT INTO        |                |
      |                   |                    | service_account_   |                |
      |                   |                    | keys (status:      |                |
      |                   |                    | active)            |                |
      |                   |                    |------------------>|                |
      |                   |                    |<------------------|                |
      |                   |                    |                    |                |
      |                   |                    | INSERT INTO        |                |
      |                   |                    | audit_events       |                |
      |                   |                    | (key_registered)   |                |
      |                   |                    |------------------>|                |
      |                   |                    |                    |                |
      |                   |                    | Invalidate key     |                |
      |                   |                    | cache              |                |
      |                   |                    |---------------------------------->|
      |                   |                    |                    |                |
      |                   | 201 {key_id: new}  |                    |                |
      |                   |<-------------------|                    |                |
      | Receives new      |                    |                    |                |
      | key_id            |                    |                    |                |
      |<------------------|                    |                    |                |
      |                   |                    |                    |                |
      |  ** Both old and new keys are now valid simultaneously **  |                |
      |  ** Deploy SDK update to use new key_id + private key  **  |                |
      |                   |                    |                    |                |
      | Step 2: Revoke    |                    |                    |                |
      | old key (after    |                    |                    |                |
      | all clients       |                    |                    |                |
      | migrated)         |                    |                    |                |
      |------------------>|                    |                    |                |
      |                   | DELETE /admin/v1/  |                    |                |
      |                   | service-accounts/  |                    |                |
      |                   | {id}/keys/{old_id} |                    |                |
      |                   |------------------->|                    |                |
      |                   |                    | UPDATE keys SET    |                |
      |                   |                    | status='revoked',  |                |
      |                   |                    | revoked_at=now()   |                |
      |                   |                    |------------------>|                |
      |                   |                    |                    |                |
      |                   |                    | INSERT INTO        |                |
      |                   |                    | audit_events       |                |
      |                   |                    | (key_revoked)      |                |
      |                   |                    |------------------>|                |
      |                   |                    |                    |                |
      |                   |                    | Invalidate key     |                |
      |                   |                    | cache              |                |
      |                   |                    |---------------------------------->|
      |                   |                    |                    |                |
      |                   | 200 {revoked}      |                    |                |
      |                   |<-------------------|                    |                |
      |<------------------|                    |                    |                |
```

**Key rotation properties:**

- **Zero-downtime:** Both old and new keys are valid during the transition window.
- **No time limit on overlap:** The overlap period is controlled by the operator, not an automatic timer. This ensures no client is cut off during deployment.
- **Audit trail:** Both registration and revocation are recorded in `audit_events` with the actor, timestamp, and affected key_id.
- **Cache invalidation:** On key registration or revocation, the gateway invalidates its Redis key cache, forcing a fresh database lookup on the next request using that service account.

---

## 5. Infrastructure Architecture

### 5.1 Deployment Topology

```
                            +-----------------------------+
                            |       Load Balancer         |
                            |    (L7, TLS termination)    |
                            |    Health: /health/ready    |
                            +------+----------+-----------+
                                   |          |
                          +--------+--+  +----+------+
                          |           |  |           |
                          | Gateway   |  | Gateway   |    (horizontally scaled,
                          | Instance  |  | Instance  |     stateless, N replicas)
                          | #1        |  | #2        |
                          +-----+-----+  +-----+-----+
                                |              |
                  +-------------+--------------+-------------+
                  |                             |             |
           +------+------+            +--------+------+ +----+------+
           |             |            |               | |           |
           | PostgreSQL  |            | Redis Cluster | | OTel      |
           | Primary     |            |  (3 masters,  | | Collector |
           |             |            |   3 replicas) | | (sidecar  |
           | + Read      |            |               | |  or DaemonSet)
           |   Replica(s)|            +---------------+ +-----------+
           +-------------+
```

**Deployment options (by scale):**

| Scale | Gateway | PostgreSQL | Redis | Deployment |
|---|---|---|---|---|
| Small (< 100 req/s) | 2 replicas | Single instance | Single instance + Sentinel | Docker Compose |
| Medium (100-1000 req/s) | 3-5 replicas | Primary + 1 read replica | 3-node Sentinel | Kubernetes (single cluster) |
| Large (> 1000 req/s) | 5-20 replicas | Primary + 2 read replicas + connection pooler (PgBouncer) | 6-node Cluster (3 masters, 3 replicas) | Kubernetes (multi-zone) |

### 5.2 Network Architecture

```
+------------------------------------------------------------------+
|                        VPC / Private Network                     |
|                                                                  |
|  +-------------------+     +----------------------------------+  |
|  | Public Subnet     |     | Private Subnet                   |  |
|  |                   |     |                                  |  |
|  | +---------------+ |     | +------------+  +-------------+  |  |
|  | | Load Balancer | |     | | Gateway    |  | Gateway     |  |  |
|  | | (public IP)   +-+-----+>| Pod #1     |  | Pod #2      |  |  |
|  | +---------------+ |     | +-----+------+  +-----+-------+  |  |
|  |                   |     |       |                |          |  |
|  +-------------------+     |       +--------+-------+          |  |
|                            |                |                  |  |
|                            | +--------------+---------------+  |  |
|                            | |              |               |  |  |
|                            | | +------+  +--+----+  +------++ |  |
|                            | | | PG   |  | Redis |  | OTel | |  |
|                            | | | (RDS)|  | Cluster| | Coll.| |  |
|                            | | +------+  +-------+  +------+ |  |
|                            | |     Data Subnet                |  |
|                            | +--------------------------------+  |
|                            +----------------------------------+  |
+------------------------------------------------------------------+
```

**Network policies:**

| Source | Destination | Port | Protocol | Direction |
|---|---|---|---|---|
| Load Balancer | Gateway Pods | 8080 | TCP (HTTP) | Ingress |
| Gateway Pods | PostgreSQL | 5432 | TCP | Egress |
| Gateway Pods | Redis | 6379 | TCP | Egress |
| Gateway Pods | OTel Collector | 4317 | TCP (gRPC) | Egress |
| Gateway Pods | LLM Provider APIs | 443 | TCP (HTTPS) | Egress |
| Admin Console | Gateway Pods | 8080 | TCP (HTTP) | Ingress |
| All other | All | * | * | **Deny** |

**TLS strategy:**

- **External traffic:** TLS terminated at the load balancer. Load balancer to gateway is plaintext within the private network (or mTLS for zero-trust environments).
- **Provider traffic:** Always HTTPS (TLS 1.3 preferred) from gateway to LLM providers.
- **Database traffic:** TLS required for PostgreSQL connections (`sslmode=require`).
- **Redis traffic:** TLS optional within the private network; required if Redis is outside the VPC.

### 5.3 Load Balancing Strategy

**Layer 7 load balancing** at the ingress:

- **Algorithm:** Least connections (optimal for variable-duration LLM requests).
- **Sticky sessions:** Not required (gateway is stateless).
- **Health check:** `GET /health/ready` every 5 seconds, 2 consecutive failures removes from pool.
- **Connection draining:** 30 seconds on instance removal (matches graceful shutdown window).
- **Timeouts:**
  - Client idle timeout: 120 seconds (allows long streaming responses)
  - Backend connect timeout: 5 seconds
  - Backend response timeout: 120 seconds

**Why least-connections over round-robin:** LLM requests have highly variable duration (200ms for a cached embedding vs 60s for a long completion). Round-robin would over-load an instance that happens to get multiple long-running requests. Least-connections naturally distributes based on actual load.

---

## 6. Integration Architecture

### 6.1 Provider Integration Patterns

**Provider Adapter trait:**

```
+-------------------+
| ProviderAdapter   |  (trait)
|                   |
| + name() -> &str  |
| + transform_req() |---> NormalizedRequest -> ProviderRequest
| + send()          |---> ProviderRequest -> ProviderResponse (non-streaming)
| + send_stream()   |---> ProviderRequest -> Stream<ProviderChunk> (streaming)
| + transform_resp()|---> ProviderResponse -> NormalizedResponse
| + map_error()     |---> ProviderError -> GatewayError
+-------------------+
         ^
         |
    +----+------+--------+--------+--------+
    |           |        |        |        |
 OpenAI   Anthropic   Gemini   Azure    vLLM
 Adapter   Adapter    Adapter  Adapter  Adapter
```

**Per-provider configuration:**

```
Provider Config (per route):
  - base_url: https://api.openai.com
  - api_key_ref: vault://providers/openai/api-key
  - timeout_seconds: 120
  - max_retries: 2
  - model_mapping: {"gpt-4o": "gpt-4o-2024-11-20"}
  - headers: {custom-header: value}
  - rate_limit: 10000 req/min (provider-side limit awareness)
```

**Provider-specific considerations:**

| Provider | Auth Mechanism | Streaming Format | Special Handling |
|---|---|---|---|
| OpenAI | `Authorization: Bearer <key>` | SSE `data: {json}` | Standard (reference implementation) |
| Anthropic | `x-api-key: <key>`, `anthropic-version: 2023-06-01` | SSE with typed events | Different message format, content blocks |
| Google Gemini | OAuth2 or API key | SSE `data: {json}` | Different content structure, safety ratings |
| Azure OpenAI | `api-key: <key>` | SSE `data: {json}` | Custom base URL per deployment, api-version param |
| vLLM | `Authorization: Bearer <key>` (if configured) | SSE `data: {json}` | OpenAI-compatible, custom base URL |

### 6.2 SDK Integration Patterns

**Python SDK architecture:**

```
+-----------------------------------------------------+
|                  Python SDK                          |
|                                                      |
|  +------------------+    +-----------------------+   |
|  | GatewayClient    |    | AsyncGatewayClient    |   |
|  | (sync)           |    | (async)               |   |
|  +--------+---------+    +----------+------------+   |
|           |                         |                |
|           +------------+------------+                |
|                        |                             |
|           +------------+------------+                |
|           |                         |                |
|  +--------+---------+    +----------+------------+   |
|  | Request Signer   |    | Transport Layer       |   |
|  |                  |    |                        |   |
|  | - Ed25519 sign   |    | - HTTPS (httpx)       |   |
|  | - SHA-256 hash   |    | - Connection pooling  |   |
|  | - Nonce gen      |    | - Retry with backoff  |   |
|  | - Timestamp      |    | - Timeout handling    |   |
|  +------------------+    +----------+------------+   |
|                                     |                |
|                          +----------+------------+   |
|                          | Streaming Iterator    |   |
|                          |                        |   |
|                          | - SSE parser           |   |
|                          | - Async iterator       |   |
|                          | - Sync iterator        |   |
|                          +------------------------+   |
+-----------------------------------------------------+
```

**SDK request signing flow:**

```
1. Serialize request body to JSON bytes
2. Compute SHA-256 hash of body bytes -> body_hash
3. Construct canonical string:
     METHOD\n
     PATH\n
     TIMESTAMP\n
     NONCE\n
     BODY_HASH
4. Sign canonical string with Ed25519 private key -> signature
5. Set headers:
     X-Service-Account-Id: <sa_id>
     X-Key-Id: <key_id>
     X-Timestamp: <unix_timestamp>
     X-Nonce: <uuid_v7>
     X-Body-SHA256: <hex(body_hash)>
     X-Signature: <base64(signature)>
6. Send HTTPS request
```

**SDK retry strategy:**

| Condition | Action | Max Retries | Backoff |
|---|---|---|---|
| Network error (ConnectionError) | Retry | 3 | Exponential: 0.5s, 1s, 2s |
| HTTP 429 (Rate Limited) | Retry with Retry-After | 3 | Use `Retry-After` header value |
| HTTP 502/503/504 | Retry | 2 | Exponential: 1s, 2s |
| HTTP 500 | Retry | 1 | 2s fixed |
| HTTP 4xx (other) | Do not retry | 0 | N/A |
| Streaming disconnect | Reconnect not supported (stateless) | 0 | N/A |

---

## 7. Security Architecture

### 7.1 Authentication Flow (Data Plane)

```
Client (SDK)                                Gateway
    |                                          |
    |  1. Construct request body               |
    |  2. SHA-256(body) -> body_hash           |
    |  3. Build canonical string               |
    |  4. Ed25519.sign(canonical, private_key) |
    |     -> signature                         |
    |  5. Set security headers                 |
    |                                          |
    |  HTTPS request with signed headers       |
    |----------------------------------------->|
    |                                          |
    |                  +-- 6. Extract headers: X-Service-Account-Id, X-Key-Id,
    |                  |      X-Timestamp, X-Nonce, X-Body-SHA256, X-Signature
    |                  |
    |                  +-- 7. Validate all headers present and well-formed
    |                  |      (fail -> 401, "missing_auth_headers")
    |                  |
    |                  +-- 8. Check X-Timestamp within +/- 300s of server time
    |                  |      (fail -> 401, "timestamp_expired")
    |                  |
    |                  +-- 9. Redis: SET nonce NX EX 600
    |                  |      (already exists -> 401, "nonce_replay")
    |                  |
    |                  +-- 10. PostgreSQL: Fetch public key for (sa_id, key_id)
    |                  |       - Key must be status = 'active'
    |                  |       - Service account must be status = 'active'
    |                  |       (not found/revoked -> 401, "invalid_key")
    |                  |
    |                  +-- 11. Recompute SHA-256 of actual request body
    |                  |       Compare with X-Body-SHA256
    |                  |       (mismatch -> 401, "body_hash_mismatch")
    |                  |
    |                  +-- 12. Reconstruct canonical string:
    |                  |       METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_HASH
    |                  |
    |                  +-- 13. Ed25519.verify(canonical, signature, public_key)
    |                  |       (fail -> 401, "signature_invalid")
    |                  |
    |                  +-- 14. Build AuthenticatedContext:
    |                         {tenant_id, service_account_id, key_id, policy_id}
    |                         Attach to request extensions
    |                                          |
    |  Proceed to Policy Engine                |
    |                                          |
```

**Security properties of this design:**

| Property | Mechanism |
|---|---|
| Authentication | Ed25519 signature proves possession of private key |
| Integrity | SHA-256 body hash + signature ensures request body was not modified |
| Replay protection | Nonce uniqueness enforced via Redis with 600s TTL |
| Freshness | Timestamp validation within +/- 300s window |
| Key isolation | Private key never leaves the client. Gateway only stores public keys. |
| Non-repudiation | Signed requests can be verified against the registered public key |

### 7.2 Authorization Flow (Admin Plane)

```
Admin User              Admin Console           Gateway (Admin API)
    |                        |                        |
    | Login                  |                        |
    |----------------------->|                        |
    |                        | POST /admin/v1/auth/   |
    |                        |   login                |
    |                        | {username, password}    |
    |                        |----------------------->|
    |                        |                        |
    |                        |             [Verify password (Argon2id)]
    |                        |             [Generate JWT:]
    |                        |             | - sub: user_id
    |                        |             | - role: admin|viewer|operator
    |                        |             | - tenant_id: <scope>
    |                        |             | - exp: now() + 15min
    |                        |             | - iat: now()
    |                        |             | - jti: unique_id
    |                        |             [Generate refresh_token:]
    |                        |             | - opaque token, stored in Redis
    |                        |             | - TTL: 24 hours
    |                        |                        |
    |                        | 200 {access_token,     |
    |                        |  refresh_token,         |
    |                        |  expires_in: 900}       |
    |                        |<-----------------------|
    |                        |                        |
    | Subsequent request     |                        |
    |----------------------->|                        |
    |                        | GET /admin/v1/tenants  |
    |                        | Authorization:         |
    |                        |   Bearer <JWT>         |
    |                        |----------------------->|
    |                        |                        |
    |                        |         [Verify JWT signature]
    |                        |         [Check expiry]
    |                        |         [Extract role + tenant_id]
    |                        |         [RBAC check:]
    |                        |         |  admin -> full access
    |                        |         |  operator -> CRUD within tenant
    |                        |         |  viewer -> read-only
    |                        |         [Tenant scope enforcement:]
    |                        |         |  Queries filtered by tenant_id
    |                        |                        |
    |                        | 200 {tenants: [...]}   |
    |                        |<-----------------------|
    |<-----------------------|                        |
```

**RBAC matrix:**

| Resource | admin | operator | viewer |
|---|---|---|---|
| Tenants (CRUD) | All tenants | Own tenant only | Own tenant (read) |
| Service Accounts | All | Within own tenant | Within own tenant (read) |
| Keys (register/revoke) | All | Within own tenant | None |
| Policies (CRUD) | All | Within own tenant | Within own tenant (read) |
| Routes (CRUD) | All | Within own tenant | Within own tenant (read) |
| Usage (query) | All | Within own tenant | Within own tenant |
| Audit (query) | All | Within own tenant | Within own tenant |
| System settings | All | None | None |

### 7.3 Key Management Lifecycle

```
State Machine for Service Account Keys:

    +----------+      Register       +---------+
    |          |--------------------->|         |
    | (none)   |                     | Active  |
    |          |                     |         |
    +----------+                     +----+----+
                                          |
                                          | Revoke
                                          |
                                     +----v----+
                                     |         |
                                     | Revoked |  (terminal state,
                                     |         |   key retained for
                                     +----+----+   audit trail)
                                          |
                                          | (never deleted)
                                          v
                                       (archived)

Key Lifecycle Events (all written to audit_events):

  1. KEY_REGISTERED   - New public key added to service account
  2. KEY_ACTIVATED    - Key marked as active (immediate on registration)
  3. KEY_REVOKED      - Key revoked by admin, no longer valid for auth
  4. KEY_EXPIRED      - Optional: auto-expiry if expiry_date was set

Rotation Timeline (zero-downtime):

  Day 0: Register new key     -> [old: active, new: active]
  Day 1-N: Roll out SDK update -> clients switch to new key
  Day N+1: Revoke old key      -> [old: revoked, new: active]
```

### 7.4 Threat Model Summary

| Threat | Attack Vector | Mitigation | Impact (if unmitigated) |
|---|---|---|---|
| Request replay | Attacker captures and replays a valid signed request | Nonce uniqueness (Redis SET NX EX 600), timestamp validation (+/- 300s) | Unauthorized LLM calls, cost inflation |
| Key compromise | Attacker obtains a service account private key | Key revocation via admin API, audit trail identifies affected window, short-lived nonces limit blast radius | Full impersonation of service account |
| Man-in-the-middle | Attacker intercepts traffic between SDK and gateway | TLS 1.3 mandatory, 0-RTT disabled, HSTS enforced | Request/response interception |
| Provider key theft | Attacker extracts LLM provider API keys from gateway | Provider keys encrypted at rest (AES-256-GCM), only decrypted in memory, never logged | Unlimited access to LLM providers |
| SQL injection | Malicious input in admin API parameters | Parameterized queries (SQLx), input validation, no string concatenation in SQL | Data exfiltration, privilege escalation |
| Rate limit bypass | Attacker uses multiple service accounts or distributed IPs | Per-tenant rate limits (in addition to per-SA), global rate limit, budget enforcement | Resource exhaustion, cost overrun |
| Denial of service | Flood of requests to exhaust gateway resources | Rate limiting, request body size limits, connection limits, auto-scaling | Service unavailability |
| Privilege escalation | Operator attempts to access other tenants' data | Tenant ID enforced at repository layer (not just API), JWT tenant scope, no cross-tenant queries possible | Data breach across tenants |
| Supply chain attack | Compromised Rust crate dependency | `cargo audit`, `cargo deny`, `cargo-geiger`, dependency pinning, SBOM generation | Arbitrary code execution in gateway |
| Admin session hijack | Attacker steals JWT or session token | Short-lived JWT (15 min), refresh token rotation, secure cookie flags (HttpOnly, Secure, SameSite=Strict) | Unauthorized admin access |

---

## 8. Scalability Architecture

### 8.1 Horizontal Scaling Strategy

```
Scaling Dimensions:

  +---------------------+
  |   Client Traffic     |
  |   (requests/sec)     |
  +----------+----------+
             |
             v
  +----------+----------+     Auto-scale trigger:
  |   Gateway Instances  |     CPU > 60% for 2 min OR
  |   (stateless)        |     active_connections > 80% of limit
  |                      |     for 2 min
  |   Min: 2 replicas    |
  |   Max: 20 replicas   |
  +----------+----------+
             |
     +-------+-------+
     |               |
     v               v
  +--+---+      +----+----+
  |  PG  |      |  Redis  |
  | Scale|      |  Scale  |
  | via  |      |  via    |
  | read |      |  cluster|
  | replicas    |  nodes  |
  +------+      +---------+
```

**Statelessness guarantee:** Gateway instances hold no request-scoped state between requests. All shared state is in PostgreSQL (durable) or Redis (ephemeral). This means any instance can handle any request, and instances can be added or removed without coordination.

**Auto-scaling policy:**

| Metric | Scale-up Threshold | Scale-down Threshold | Cooldown |
|---|---|---|---|
| CPU utilization | > 60% for 2 min | < 30% for 5 min | 3 min up, 5 min down |
| Active connections | > 80% of limit for 2 min | < 40% for 5 min | 3 min up, 5 min down |
| Request queue depth | > 100 pending for 1 min | < 10 for 5 min | 3 min up, 5 min down |

### 8.2 Database Scaling

**Read replica strategy:**

```
                     Writes (INSERT, UPDATE)
                            |
                            v
                    +-------+--------+
                    |   PG Primary   |
                    |                |
                    +-------+--------+
                            |
                   Streaming Replication
                            |
               +------------+-------------+
               |                          |
       +-------+--------+       +--------+-------+
       |   PG Replica    |       |   PG Replica    |
       |   #1            |       |   #2            |
       +----------------+       +----------------+

  Read routing:
    - Admin queries (tenant list, SA list) -> Replica
    - Usage queries (dashboards, export)   -> Replica
    - Auth key lookup                      -> Primary (consistency)
    - Policy lookup                        -> Primary (consistency)
    - Write (usage_events, audit)          -> Primary
```

**Usage_events partitioning:**

```sql
-- Partition by month for efficient time-range queries and retention management
CREATE TABLE usage_events (
    id          UUID        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id   UUID        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- ... other columns
) PARTITION BY RANGE (created_at);

-- Create monthly partitions
CREATE TABLE usage_events_2026_04 PARTITION OF usage_events
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');

-- Retention: detach and archive old partitions
ALTER TABLE usage_events DETACH PARTITION usage_events_2025_04;
-- Export to object storage, then DROP
```

**Benefits of partitioning:**

- Time-range queries scan only relevant partitions (partition pruning).
- Retention is achieved by detaching and dropping old partitions (instant, no DELETE overhead).
- Vacuum runs per-partition, avoiding long-running vacuums on a large table.
- Index maintenance is per-partition, keeping index sizes manageable.

**Connection pooling (PgBouncer for large scale):**

For deployments beyond 5 gateway instances, introduce PgBouncer in transaction pooling mode between the gateway and PostgreSQL to prevent connection exhaustion:

```
Gateway (20 conns each) x 10 instances = 200 connections
    |
    v
PgBouncer (pool_size=50, max_client_conn=400)
    |
    v
PostgreSQL (max_connections=100)
```

### 8.3 Redis Scaling

**Decision matrix:**

| Scale | Architecture | Rationale |
|---|---|---|
| Small (< 100 req/s) | Single Redis + Sentinel (3 nodes) | HA with automatic failover, simple operations |
| Medium (100-1000 req/s) | Redis Sentinel (3 nodes: 1 primary, 2 replicas) | Read distribution for health probes, HA |
| Large (> 1000 req/s) | Redis Cluster (6 nodes: 3 masters, 3 replicas) | Sharded writes for rate limiting and nonce storage |

**Why Cluster over Sentinel at large scale:**

- **Rate limiting:** With thousands of requests per second, `INCR` operations on rate limit keys become a write hotspot. Redis Cluster distributes keys across masters via hash slots.
- **Nonce storage:** Each request creates a new nonce entry (`SET NX EX`). At 10,000 req/s, that is 10,000 writes/sec -- beyond what a single master can sustain with durability guarantees.
- **Fred client handles cluster natively:** Fred caches cluster state, routes commands to the correct shard, and handles MOVED/ASK redirections transparently.

**Key distribution strategy:**

```
Nonce keys:     nonce:{nonce_value}          -> distributed by nonce hash
Rate limits:    rl:{tenant_id}:{window}      -> distributed by tenant_id hash
Sessions:       session:{token}              -> distributed by token hash
Health:         health:{provider}            -> distributed by provider hash
Config cache:   config:{key}                 -> distributed by key hash
```

### 8.4 Connection Pooling Summary

| Connection Type | Per Instance | Strategy | Max Total (10 instances) |
|---|---|---|---|
| PG write pool | 15 connections | SQLx built-in pool | 150 (use PgBouncer above 5 instances) |
| PG read pool | 5 connections | Separate pool for read replica | 50 |
| Redis pool | 6 connections | Fred round-robin pool | 60 |
| Reqwest (per provider) | 32 idle per host | Reqwest built-in pool | 320 per provider |
| OTel exporter | 2 gRPC connections | Tonic client pool | 20 |

---

## 9. Reliability Architecture

### 9.1 Retry Patterns

**Retry strategy for provider calls:**

```
                     Request
                        |
                        v
                  +-----------+
                  | Attempt 1 |----> Provider (primary)
                  +-----------+
                        |
                   Success? ----Yes----> Return response
                        |
                       No (5xx/timeout/network error)
                        |
                        v
                  [Wait: 500ms * 2^0 + jitter]
                        |
                        v
                  +-----------+
                  | Attempt 2 |----> Provider (same, if not circuit-broken)
                  +-----------+
                        |
                   Success? ----Yes----> Return response
                        |
                       No
                        |
                        v
                  [Wait: 500ms * 2^1 + jitter]
                        |
                        v
                  +-----------+
                  | Attempt 3 |----> Provider (same or fallback)
                  +-----------+
                        |
                   Success? ----Yes----> Return response
                        |
                       No
                        |
                        v
                  Return 502 Provider Error
```

**Retry configuration:**

| Parameter | Value | Rationale |
|---|---|---|
| Max retries | 2 (3 total attempts) | Balance between reliability and latency |
| Base delay | 500ms | Short enough for interactive use |
| Backoff | Exponential (500ms, 1s, 2s) | Standard exponential backoff |
| Jitter | Random 0-50% of delay | Prevent thundering herd |
| Retry conditions | HTTP 500, 502, 503, 504, connection error, timeout | Transient errors only |
| Non-retryable | HTTP 400, 401, 403, 404, 422, 429 | Client errors, rate limits (use Retry-After for 429) |

**Streaming retry:** Streaming requests are NOT retried mid-stream. If the stream fails after the first SSE event is sent to the client, the gateway emits a `response.error` event and closes the stream. The client SDK can choose to retry the full request.

### 9.2 Circuit Breaker Design

```
Circuit Breaker State Machine (per provider):

                          Success
                     +------------------+
                     |                  |
                     v                  |
                 +---+----+        +----+---+
          +----->| CLOSED  |------->| CLOSED |
          |      |         | fail   |        |
          |      +---------+ count  +---+----+
          |                              |
          |                     Failure threshold
          |                     reached (5 failures
          |                     in 60s window)
          |                              |
          |                              v
          |                         +----+----+
          |     Success on          |         |
          |     probe request       |  OPEN   |  <-- All requests fail-fast
          |                         |         |      with 503 for this provider
          |                         +----+----+
          |                              |
          |                     After cooldown period
          |                     (30 seconds)
          |                              |
          |                              v
          |                      +-------+------+
          +----------------------+ HALF-OPEN    |
                                 |              |  <-- Allow 1 probe request
                     Probe fails |              |      through to test recovery
                     +---------->+--------------+
                     |
                     v
                 +---+----+
                 |  OPEN  |
                 +---------+
```

**Circuit breaker configuration (per provider):**

| Parameter | Value |
|---|---|
| Failure threshold | 5 failures within 60 seconds |
| Failure conditions | HTTP 500, 502, 503, timeout, connection error |
| Cooldown period (open -> half-open) | 30 seconds |
| Probe count (half-open) | 1 request |
| Success threshold to close | 1 successful probe |
| Metrics emitted | `llmsmartgate_circuit_breaker_state{provider}` |

**Implementation:** Using `tower-resilience` circuit breaker middleware, composed per-provider in the `ServiceBuilder` chain:

```
timeout(120s) -> circuit_breaker -> retry(3) -> reqwest_client
```

### 9.3 Fallback Chain Execution

```
Route Configuration Example:
  model_alias: "gpt-4o"
  routes:
    - provider: openai,    priority: 1, weight: 100
    - provider: azure,     priority: 2, weight: 100  (fallback)
    - provider: anthropic, priority: 3, weight: 100  (second fallback, model: claude-sonnet-4-20250514)

Execution Flow:

    Request (model: "gpt-4o")
         |
         v
    Resolve route -> [openai(P1), azure(P2), anthropic(P3)]
         |
         v
    +-- Try openai (P1)
    |     |
    |     +-- Circuit CLOSED? --> Yes --> Send request
    |     |                               |
    |     |                          Success? -> Return response
    |     |                               |
    |     |                              No (after retries exhausted)
    |     |                               |
    |     +-- Circuit OPEN? --> Yes --+   |
    |                                 |   |
    |     <------ Skip to next -------+---+
    |
    +-- Try azure (P2)
    |     |
    |     +-- (same pattern as above)
    |     |
    |     +-- Success? -> Return response
    |     |
    |     +-- Fail -> continue
    |
    +-- Try anthropic (P3)
    |     |
    |     +-- Transform request to Anthropic format
    |     |   (model mapping: gpt-4o -> claude-sonnet-4-20250514)
    |     |
    |     +-- Success? -> Return response (transformed back to OpenAI format)
    |     |
    |     +-- Fail -> All providers exhausted
    |
    +-- Return 502 "All providers failed"
         (Include: attempted_providers, failure_reasons in error response)
```

### 9.4 Graceful Degradation

**Degradation levels:**

| Level | Condition | Behavior |
|---|---|---|
| Normal | All systems operational | Full functionality |
| Degraded (Redis) | Redis unreachable | Nonce check skipped (log warning), rate limits not enforced (log warning), request processing continues. Timestamp validation still enforced. |
| Degraded (Read Replica) | PG read replica down | All reads route to primary. Admin dashboard may be slower. |
| Degraded (Provider) | 1+ providers circuit-broken | Fallback chain active. Affected models may have higher latency. Alert fired. |
| Degraded (All Providers) | All routes for a model exhausted | Return 502 with clear error. Budget not charged. |
| Critical (PG Primary) | PostgreSQL primary unreachable | Auth fails (cannot fetch keys unless cached). Admin API returns 503. Data plane returns 503. |
| Shutdown | SIGTERM received | Stop accepting new connections, drain in-flight (30s), flush telemetry, exit. |

**Redis degradation strategy:**

When Redis is unavailable, the gateway enters a degraded mode rather than failing completely:

```
Redis Health Check (every 5s):
  |
  +-- Healthy: Normal operation
  |
  +-- Unhealthy:
        |
        +-- Nonce check: SKIP (accept replay risk for availability)
        |   Log WARN: "nonce check bypassed, redis unavailable"
        |
        +-- Rate limiting: SKIP (accept burst risk for availability)
        |   Log WARN: "rate limiting bypassed, redis unavailable"
        |
        +-- Provider health: USE STALE (last known state)
        |
        +-- Emit metric: llmsmartgate_redis_degraded = 1
        +-- Fire alert: "Redis unavailable, degraded mode active"
```

---

## 10. Observability Architecture

### 10.1 Trace Propagation Design

```
End-to-End Trace Flow:

Client SDK              Gateway                    Provider API
    |                      |                            |
    | traceparent:         |                            |
    | (optional, from      |                            |
    |  client's own trace) |                            |
    |--------------------->|                            |
    |                      |                            |
    |               [If traceparent present:]            |
    |               [  Continue trace as child span]    |
    |               [If absent:]                        |
    |               [  Generate new trace_id]           |
    |                      |                            |
    |               [Create root span:]                 |
    |               [  gateway.request]                 |
    |               [  attributes:]                     |
    |               [    request_id, tenant_id,]        |
    |               [    service_account_id,]           |
    |               [    http.method, http.route]       |
    |                      |                            |
    |               [Child spans:]                      |
    |               [  auth.verify    (2ms)]            |
    |               [  policy.evaluate (1ms)]           |
    |               [  routing.resolve (0.5ms)]         |
    |               [  provider.call   (1200ms)]        |
    |               [    ^-- propagate traceparent ---->|
    |               [  usage.persist   (async)]         |
    |                      |                            |
    |               [Set response header:]              |
    |               [  X-Request-Id: req_abc123]        |
    |               [  (traceparent NOT returned]       |
    |               [   to client for security)]        |
    |                      |                            |
    |  Response            |                            |
    |<---------------------|                            |
```

**Trace sampling strategy:**

| Traffic Type | Sampling Rate | Rationale |
|---|---|---|
| Error responses (4xx, 5xx) | 100% | Always trace errors for debugging |
| Slow requests (> 10s) | 100% | Capture performance outliers |
| Normal requests | 10% | Balance storage cost vs visibility |
| Admin API requests | 100% | Low volume, high audit value |
| Health checks | 0% | Noise, no diagnostic value |

**Implementation with `tracing-opentelemetry`:**

```
tracing-subscriber (layer stack):
    |
    +-- fmt::Layer (JSON stdout)           -> Local log aggregation
    |
    +-- OpenTelemetryLayer                 -> OTel Collector (OTLP/gRPC)
    |     |
    |     +-- BatchSpanProcessor           -> Batch export every 5s or 512 spans
    |     +-- TraceContextPropagator       -> W3C traceparent injection/extraction
    |     +-- ParentBasedSampler           -> Respect upstream sampling decision
    |           +-- TraceIdRatioBased(0.1) -> 10% sampling for root spans
    |
    +-- EnvFilter                          -> LLMSG_LOG_LEVEL=info
```

### 10.2 Metrics Pipeline

```
Gateway Instance                OTel Collector              Backend
    |                               |                         |
    | prometheus::Registry          |                         |
    | (in-process metrics)          |                         |
    |                               |                         |
    |  Option A: Pull-based         |                         |
    |  /metrics endpoint            |                         |
    |<--------- scrape -------------|                         |
    |                               |--- push to ----------->|
    |                               |    Prometheus           | Prometheus
    |                               |    remote write         |
    |                               |                         |
    |  Option B: Push-based         |                         |
    |  OTLP/gRPC metrics export     |                         |
    |------- push every 30s ------->|                         |
    |                               |--- convert & push ----->|
    |                               |                         |
    |                               |                         |
    |  Custom metrics flow:         |                         |
    |                               |                         |
    |  [Request completes]          |                         |
    |  -> increment counter         |                         |
    |  -> observe histogram         |                         |
    |  -> update gauge              |                         |
    |                               |                         |
```

**Recommended approach:** Pull-based (Prometheus scrape) via a `/metrics` endpoint on the gateway. This is simpler to operate and debug than push-based OTLP metrics.

**Metric export implementation:**

```rust
// Expose /metrics endpoint using prometheus crate
let metrics_handle = PrometheusHandle::new();
let app = Router::new()
    .route("/metrics", get(move || ready(metrics_handle.render())));
```

### 10.3 Log Aggregation

```
Gateway Instance              Collection            Storage & Query
    |                            |                        |
    | JSON logs to stdout        |                        |
    |--------------------------->|                        |
    |                            |                        |
    |  [Collection options:]     |                        |
    |                            |                        |
    |  Option A (Kubernetes):    |                        |
    |  Fluentd/Fluent Bit        |                        |
    |  DaemonSet reads           |  Forward to ---------> | Loki
    |  container stdout          |  Loki via API          | (label-based
    |                            |                        |  indexing)
    |  Option B (Docker):        |                        |
    |  Docker log driver         |  Forward to ---------> | Loki
    |  (fluentd or loki)         |  Loki directly         |
    |                            |                        |
    |  Option C (OTel):          |                        |
    |  OTLP log export           |                        |
    |  from tracing-subscriber   |  OTel Collector -----> | Loki
    |  (OpenTelemetry logs       |  (OTLP -> Loki        |
    |   are now stable in Rust)  |   exporter)           |
    |                            |                        |

Log Labels (for Loki indexing):

  {service="llmsmartgate-gateway", instance="pod-xyz", level="ERROR"}

Queryable Fields (via JSON parsing):

  | Field         | Example                           | Indexed |
  |---------------|-----------------------------------|---------|
  | level         | INFO, WARN, ERROR                 | Yes     |
  | target        | llmsmartgate::auth::verifier      | Yes     |
  | request_id    | req_abc123                        | No (filter) |
  | trace_id      | 4bf92f3577b34da6a3ce929d...       | No (filter) |
  | tenant_id     | tenant_xyz                        | No (filter) |
  | provider      | openai                            | No (filter) |
  | duration_ms   | 1432                              | No (filter) |
```

**Correlation pattern:**

All three signals (traces, metrics, logs) are correlated via:

- **trace_id:** Present in all log entries within a request, links to the trace in Tempo/Jaeger.
- **request_id:** Gateway-generated ID present in logs, response headers, and usage_events for cross-referencing.
- **exemplars:** Histogram metrics include trace_id exemplars, allowing drill-down from a Grafana metric panel to the specific trace.

### 10.4 Alerting Strategy

```
Alert Pipeline:

  Prometheus (metrics)                  Alertmanager               Notification
       |                                     |                      Channels
       |  PrometheusRule evaluation           |                         |
       |  (every 15s)                         |                         |
       |                                     |                         |
       |  [Rule fires]                       |                         |
       |------------------------------------>|                         |
       |                                     |                         |
       |                          [Group alerts by:]                   |
       |                          | - service                          |
       |                          | - severity                         |
       |                          | - alertname                        |
       |                          [Wait 30s for grouping]              |
       |                          [Deduplicate]                        |
       |                          [Inhibit lower-severity]             |
       |                                     |                         |
       |                          [Route by severity:]                 |
       |                          | critical -> PagerDuty + Slack      |
       |                          | warning  -> Slack                  |
       |                          | info     -> Slack (low-urgency)    |
       |                                     |------------------------>|
       |                                     |                         |
```

**Alert tiers:**

| Tier | SLA | Examples | Notification |
|---|---|---|---|
| P1 (Critical) | Respond in 15 min | Total gateway down, PG primary unreachable, auth failure spike | PagerDuty page + Slack #incidents |
| P2 (Warning) | Respond in 1 hour | High latency, single provider down, budget threshold, pool exhaustion | Slack #alerts |
| P3 (Info) | Next business day | Certificate expiring in 14 days, dependency update available | Slack #ops-info |

**Runbook links:** Every alert definition MUST include a `runbook_url` annotation linking to the troubleshooting guide for that specific alert.

---

## References

### Research Sources

- [Top 5 LLM Gateways in 2025: Architecture, Features, and Practical Selection Guide](https://dev.to/kuldeep_paul/top-5-llm-gateways-in-2025-architecture-features-and-a-practical-selection-guide-56nh)
- [AI Gateway Deep Dive (2026): Architecture, Product Comparison, and Production Practices](https://jimmysong.io/blog/ai-gateway-in-depth/)
- [Top 5 LLM Gateways in 2026 for Enterprise-Grade Reliability and Scale](https://www.getmaxim.ai/articles/top-5-llm-gateways-in-2026-for-enterprise-grade-reliability-and-scale/)
- [LLM Gateway On-Premise Infrastructure: An Overview](https://www.truefoundry.com/blog/llm-gateway-on-premise-infrastructure)
- [C4 Model -- Microservices](https://c4model.com/abstractions/microservices)
- [C4 Container Diagrams -- Microservices Architecture](https://visual-c4.com/blog/c4-container-microservices-examples)
- [Diagramming Microservices With the C4 Model](https://dzone.com/articles/diagramming-microservices-with-the-c4-model)
- [Tower Resilience -- Resilience Middleware for Tower](https://docs.rs/tower-resilience/latest/tower_resilience/)
- [Rust Web Frameworks in 2026](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2)
- [Axum vs Actix-web vs Rocket: Rust Web Framework Comparison 2026](https://reintech.io/blog/axum-vs-actix-web-vs-rocket-rust-framework-comparison-2026)
- [Understanding Redis High Availability: Cluster vs. Sentinel](https://medium.com/@khandelwal.praful/understanding-redis-high-availability-cluster-vs-sentinel-420ecaac3236)
- [Fred Redis Client for Rust](https://github.com/aembke/fred.rs)
- [OpenTelemetry Rust SDK](https://opentelemetry.io/docs/languages/rust/)
- [W3C Trace Context Propagation -- OpenTelemetry Rust](https://uptrace.dev/get/opentelemetry-rust/propagation)
- [10 Game-Changing Strategies to Supercharge API Gateway Performance](https://zuplo.com/learning-center/strategies-to-supercharge-your-api-gateway-performance)
- [LLM Gateway vs Direct API Calls: Benchmarking Latency & Uptime](https://www.requesty.ai/blog/llm-gateway-vs-direct-api-calls-benchmarking-latency-uptime-1751654050)
- [OWASP API Security Top 10](https://owasp.org/API-Security/)
- [OWASP API Security Testing Checklist 2026](https://accuknox.com/blog/owasp-api-security-top-10-the-complete-testing-checklist-2026)
