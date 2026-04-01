# LLMSmartGate -- Architecture & Technology Standards

> **Doc ID:** `ARC-001`  
> **Version:** 1.0  
> **Date:** 2026-04-01  
> **Status:** Draft  
> **Authors:** Solution Architecture Team  
> **Classification:** Internal -- Engineering

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Solution Architect |
| **Perspective** | Technology decisions, coding standards, governance |
| **Primary Audience** | Rust Developer, Svelte Developer, DevOps, Tech Lead |
| **Secondary Audience** | QA, Security Team |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| `[MANDATORY]` | Non-negotiable standard -- must be followed |
| `[RECOMMENDED]` | Best practice -- deviate only with documented justification |
| `[OPTIONAL]` | Suggested approach -- team discretion |
| `[RESEARCH]` | Decision backed by cited benchmark or industry source |
| `[BANNED]` | Explicitly prohibited -- do not use |

### Reading Order Suggestion

1. Start with **Architecture Principles** to understand the "why"
2. Read **Technology Stack Decisions** for the "what"
3. Reference **Coding/Security/Observability Standards** during implementation
4. Use **Performance/Data/Deployment Standards** as checklists before PR review

### Document Relationships

```
PRD-001                -- What & Why
  ├── ARC-001 (this)   -- Standards & Governance
  ├── HLD-001          -- High-Level Design
  ├── LLD-BE-001       -- Backend references ARC-001 for tech choices
  ├── LLD-FE-001       -- Frontend references ARC-001 for tech choices
  └── TST-001          -- Tests verify ARC-001 standards compliance
```

---

## Table of Contents

- [Part 1: Architecture Standards](#part-1-architecture-standards)
  - [1. Architecture Principles](#1-architecture-principles)
  - [2. System Context Diagram (C4 Level 1)](#2-system-context-diagram-c4-level-1)
  - [3. Container Diagram (C4 Level 2)](#3-container-diagram-c4-level-2)
  - [4. Component Diagram (C4 Level 3)](#4-component-diagram-c4-level-3)
  - [5. Data Flow Diagrams](#5-data-flow-diagrams)
  - [6. Deployment Architecture](#6-deployment-architecture)
  - [7. Network Architecture & Security Zones](#7-network-architecture--security-zones)
  - [8. Scalability Strategy](#8-scalability-strategy)
  - [9. Disaster Recovery & High Availability](#9-disaster-recovery--high-availability)
  - [10. Performance Architecture](#10-performance-architecture)
- [Part 2: Technology Standards](#part-2-technology-standards)
  - [11. Technology Stack Decision Matrix](#11-technology-stack-decision-matrix)
  - [12. Rust Crate Standards](#12-rust-crate-standards)
  - [13. SvelteKit Standards](#13-sveltekit-standards)
  - [14. Python SDK Standards](#14-python-sdk-standards)
  - [15. Database Standards](#15-database-standards)
  - [16. API Design Standards](#16-api-design-standards)
  - [17. Security Standards](#17-security-standards)
  - [18. Observability Standards](#18-observability-standards)
  - [19. Code Quality Standards](#19-code-quality-standards)
  - [20. Container & Deployment Standards](#20-container--deployment-standards)
  - [21. Git & Branching Standards](#21-git--branching-standards)
- [Appendix A: Research Sources](#appendix-a-research-sources)

---

# Part 1: Architecture Standards

---

## 1. Architecture Principles

| # | Principle | Rationale |
|---|-----------|-----------|
| AP-1 | **Security by Default** | Every request passes through authentication, authorization, and policy enforcement. TLS is mandatory. Secrets never appear in logs or responses. Ed25519 asymmetric signing eliminates shared-secret risks. This is non-negotiable for a system that proxies API keys to LLM providers. |
| AP-2 | **Stateless Data Plane** | The gateway data plane holds no session state between requests. All state lives in PostgreSQL (durable) or Redis/Valkey (ephemeral). This enables horizontal scaling by adding identical gateway instances behind a load balancer. |
| AP-3 | **OpenAI-Compatible Contract** | The data plane API conforms to the OpenAI API schema so existing client code works without modification. Divergence is only permitted when extending (not replacing) the contract. This maximizes developer adoption. |
| AP-4 | **Observability as a First-Class Concern** | Every request receives a `request_id` and `trace_id` propagated through all layers. Structured logs, metrics, and distributed traces are emitted from day one, not bolted on later. Correlation IDs enable end-to-end debugging across SDK, gateway, and provider. |
| AP-5 | **Fail-Safe Over Fail-Open** | On ambiguous failure (e.g., policy engine unreachable, Redis down), the gateway denies the request rather than silently bypassing controls. Safety defaults protect budget and access boundaries. |
| AP-6 | **Separation of Planes** | The data plane (request processing) and control plane (admin management) are logically separated with distinct API surfaces, authentication mechanisms, and deployment concerns. This allows independent scaling and security hardening. |
| AP-7 | **Provider Abstraction** | All LLM provider interactions flow through a common adapter trait. Adding a new provider requires implementing the trait, not modifying core request processing logic. |
| AP-8 | **Append-Only Audit Trail** | Usage events and audit events are immutable append-only records. This supports compliance, cost analysis, and forensic investigation. |
| AP-9 | **Minimal Overhead** | The gateway must add less than 200ms p95 latency overhead. Async I/O, connection pooling, and zero-copy streaming are required to achieve this. Rust's ownership model eliminates GC pauses that would violate this target. |
| AP-10 | **Defense in Depth** | Security controls are layered: network segmentation, TLS, request signing, nonce replay protection, policy enforcement, rate limiting, and audit logging. No single control's failure compromises the system. |

---

## 2. System Context Diagram (C4 Level 1)

The system context identifies LLMSmartGate and the external actors that interact with it.

```
+------------------------------------------------------------------+
|                        External Systems                           |
+------------------------------------------------------------------+

  [Backend Service]         [Platform Engineer]       [Security/Ops]
   Uses Python SDK           Uses Admin Console        Reviews Audit
        |                         |                        |
        | HTTPS (signed)          | HTTPS (JWT)            | HTTPS (JWT)
        v                         v                        v
+------------------------------------------------------------------+
|                                                                    |
|                     LLMSmartGate System                            |
|                                                                    |
|   Self-hosted LLM API Gateway providing unified, secure,          |
|   policy-governed access to multiple LLM providers.                |
|                                                                    |
+------------------------------------------------------------------+
        |                    |                     |
        | HTTPS              | HTTPS               | HTTPS
        v                    v                     v
  [OpenAI API]        [Anthropic API]        [Google Gemini API]
  [Azure OpenAI]      [vLLM / Local]
```

**Actors:**

| Actor | Type | Description |
|-------|------|-------------|
| Backend Service | External System | Application workloads that invoke LLM APIs through the Python SDK. Authenticates via Ed25519 request signing. |
| Platform Engineer | Person | Configures tenants, service accounts, policies, routes, and provider settings through the Admin Console. |
| Security / Ops | Person | Reviews audit logs, monitors usage metrics, manages key rotation, and investigates anomalies. |
| LLM Providers | External Systems | OpenAI, Anthropic, Google Gemini, Azure OpenAI, and self-hosted vLLM instances that serve the actual LLM inference. |

---

## 3. Container Diagram (C4 Level 2)

The container diagram decomposes LLMSmartGate into deployable units.

```
+------------------------------------------------------------------------+
|                         LLMSmartGate System                             |
|                                                                         |
|  +--------------------+         +--------------------+                  |
|  |  Gateway Service   |         |  Admin Console     |                  |
|  |  (Rust / Axum)     |         |  (SvelteKit)       |                  |
|  |                    |         |                    |                  |
|  |  - Data Plane API  |         |  - Dashboard       |                  |
|  |  - Admin API       |<--------+  - Config CRUD     |                  |
|  |  - SSE Streaming   |  REST   |  - Usage Explorer  |                  |
|  |  - Auth Middleware  |         |  - Audit Viewer    |                  |
|  +--------+-----------+         +--------------------+                  |
|           |        |                                                    |
|           |        |                                                    |
|     +-----v---+ +--v-----------+                                        |
|     |PostgreSQL| |Redis / Valkey|                                       |
|     |          | |              |                                       |
|     |- Tenants | |- Nonces      |                                       |
|     |- Accounts| |- Rate limits |                                       |
|     |- Policies| |- Health cache|                                       |
|     |- Routes  | |- Route cache |                                       |
|     |- Usage   | |              |                                       |
|     |- Audit   | |              |                                       |
|     +----------+ +--------------+                                       |
+------------------------------------------------------------------------+
```

**Container Descriptions:**

| Container | Technology | Responsibility |
|-----------|------------|----------------|
| **Gateway Service** | Rust, Axum 0.8, Tokio | Core gateway process. Serves the Data Plane API (OpenAI-compatible) and Admin API. Handles authentication, policy enforcement, routing, provider adaptation, streaming relay, usage metering, and audit logging. Single binary deployment. |
| **Admin Console** | SvelteKit 2, Svelte 5, shadcn-svelte, TypeScript | Browser-based management UI. Communicates exclusively with the Admin API. Provides dashboards, CRUD for configuration entities, usage exploration, and audit log viewing. Deployed as a static build or SSR Node process. |
| **PostgreSQL** | PostgreSQL 17+ | Primary durable data store for all control plane entities (tenants, service accounts, keys, policies, routes) and append-only data plane records (usage events, audit events). |
| **Redis / Valkey** | Valkey 8 (preferred) or Redis 7.4 | Ephemeral data store for nonce replay protection (TTL-based), sliding window rate limiting, provider health cache, and route resolution cache. |

---

## 4. Component Diagram (C4 Level 3)

### 4.1 Gateway Service Components

```
+------------------------------------------------------------------+
|                      Gateway Service                              |
|                                                                    |
|  +-------------------+    +-------------------+                    |
|  | API Layer         |    | Admin API Layer   |                    |
|  | (data_plane.rs)   |    | (admin.rs)        |                    |
|  | - /v1/chat/compl. |    | - /admin/tenants  |                    |
|  | - /v1/responses   |    | - /admin/sa       |                    |
|  | - /v1/embeddings  |    | - /admin/keys     |                    |
|  | - /v1/models      |    | - /admin/policies |                    |
|  +--------+----------+    | - /admin/routes   |                    |
|           |               | - /admin/usage    |                    |
|           v               | - /admin/audit    |                    |
|  +-------------------+    +--------+----------+                    |
|  | Middleware Stack   |             |                               |
|  | - RequestId        |             |                               |
|  | - Tracing          |             |                               |
|  | - Compression      |             |                               |
|  | - CORS             |             |                               |
|  | - Timeout          |             |                               |
|  +--------+----------+             |                               |
|           |                        |                               |
|           v                        |                               |
|  +-------------------+             |                               |
|  | Auth Module        |             |                               |
|  | - Signature verify |             |                               |
|  | - Nonce check      |             |                               |
|  | - Key resolution   |             |                               |
|  | - Timestamp skew   |             |                               |
|  +--------+----------+             |                               |
|           |                        |                               |
|           v                        |                               |
|  +-------------------+             |                               |
|  | Policy Engine      |             |                               |
|  | - Model access     |             |                               |
|  | - Token limits     |             |                               |
|  | - Feature flags    |             |                               |
|  | - Budget check     |             |                               |
|  +--------+----------+             |                               |
|           |                        |                               |
|           v                        |                               |
|  +-------------------+             |                               |
|  | Routing Engine     |             |                               |
|  | - Alias resolver   |             |                               |
|  | - Priority sort    |             |                               |
|  | - Fallback chain   |             |                               |
|  | - Retry policy     |             |                               |
|  +--------+----------+             |                               |
|           |                        |                               |
|           v                        |                               |
|  +-------------------+             |                               |
|  | Provider Adapters  |             |                               |
|  | - OpenAI           |             |                               |
|  | - Anthropic        |             |                               |
|  | - Gemini           |             |                               |
|  | - Azure OpenAI     |             |                               |
|  | - vLLM             |             |                               |
|  +--------+----------+             |                               |
|           |                        |                               |
|           v                        |                               |
|  +-------------------+    +--------v----------+                    |
|  | Streaming Engine   |    | Storage Layer     |                    |
|  | - SSE relay        |    | - PG repositories |                    |
|  | - Event normalize  |    | - Redis client    |                    |
|  | - Disconnect detect|    | - Connection pools |                   |
|  | - Upstream cancel  |    +-------------------+                    |
|  +-------------------+                                             |
|           |                                                        |
|           v                                                        |
|  +-------------------+    +-------------------+                    |
|  | Usage Metering     |    | Audit Logger      |                    |
|  | - Token counting   |    | - Event capture   |                    |
|  | - Cost estimation  |    | - Async persist   |                    |
|  | - Async persist    |    +-------------------+                    |
|  +-------------------+                                             |
|                                                                    |
|  +-------------------+                                             |
|  | Observability      |                                            |
|  | - tracing spans    |                                            |
|  | - metrics counters |                                            |
|  | - OTLP export      |                                            |
|  +-------------------+                                             |
+------------------------------------------------------------------+
```

### 4.2 Admin Console Components

```
+------------------------------------------------------------------+
|                      Admin Console (SvelteKit)                     |
|                                                                    |
|  +-------------------+    +-------------------+                    |
|  | Pages (Routes)     |    | Shared Components |                    |
|  | - /dashboard       |    | - DataTable       |                    |
|  | - /tenants         |    | - FormDialog      |                    |
|  | - /service-accounts|    | - Charts          |                    |
|  | - /keys            |    | - Sidebar         |                    |
|  | - /policies        |    | - Breadcrumbs     |                    |
|  | - /routes          |    | - ErrorBoundary   |                    |
|  | - /usage           |    +-------------------+                    |
|  | - /audit           |                                            |
|  +--------+----------+    +-------------------+                    |
|           |               | State Management  |                    |
|           v               | - Runes ($state)  |                    |
|  +-------------------+    | - Reactive Classes|                    |
|  | API Client Layer   |    | - Server Loads    |                    |
|  | - fetch wrappers   |    +-------------------+                    |
|  | - auth interceptor |                                            |
|  | - error handling   |    +-------------------+                    |
|  | - type-safe DTOs   |    | UI Primitives     |                    |
|  +-------------------+    | - shadcn-svelte   |                    |
|                           | - bits-ui         |                    |
|                           | - Tailwind CSS v4 |                    |
|                           +-------------------+                    |
+------------------------------------------------------------------+
```

### 4.3 Python SDK Components

```
+------------------------------------------------------------------+
|                      Python SDK (llmsmartgate)                     |
|                                                                    |
|  +-------------------+    +-------------------+                    |
|  | GatewayClient     |    | AsyncGatewayClient|                    |
|  | (sync interface)  |    | (async interface) |                    |
|  +--------+----------+    +--------+----------+                    |
|           |                        |                               |
|           +----------+-------------+                               |
|                      |                                             |
|           +----------v----------+                                  |
|           | Request Signer      |                                  |
|           | - Ed25519 signing   |                                  |
|           | - Canonical string  |                                  |
|           | - Nonce generation  |                                  |
|           +----------+----------+                                  |
|                      |                                             |
|           +----------v----------+                                  |
|           | Transport Layer     |                                  |
|           | - httpx Client      |                                  |
|           | - Retry logic       |                                  |
|           | - Timeout handling  |                                  |
|           +----------+----------+                                  |
|                      |                                             |
|           +----------v----------+                                  |
|           | Streaming Iterator  |                                  |
|           | - SSE parser        |                                  |
|           | - Event normalizer  |                                  |
|           +---------------------+                                  |
|                                                                    |
|           +---------------------+                                  |
|           | Models / Types      |                                  |
|           | - Pydantic v2       |                                  |
|           | - Request / Response|                                  |
|           +---------------------+                                  |
+------------------------------------------------------------------+
```

---

## 5. Data Flow Diagrams

### 5.1 Data Plane -- Non-Streaming Request Lifecycle

```
Client SDK                  Gateway Service                          LLM Provider
    |                            |                                        |
    |  1. Build request          |                                        |
    |  2. Compute body SHA-256   |                                        |
    |  3. Build canonical string |                                        |
    |  4. Sign with Ed25519      |                                        |
    |                            |                                        |
    |--- POST /v1/chat/compl. -->|                                        |
    |    (signed headers + body) |                                        |
    |                            |                                        |
    |                       5. Generate request_id, trace_id              |
    |                       6. Validate JSON schema                       |
    |                       7. Extract auth headers                       |
    |                       8. Fetch service account + key (PG)           |
    |                       9. Rebuild canonical string                   |
    |                      10. Verify Ed25519 signature                   |
    |                      11. Check timestamp skew (<=300s)              |
    |                      12. Check nonce uniqueness (Redis)             |
    |                      13. Store nonce with TTL=300s (Redis)          |
    |                       --- Auth Context Established ---              |
    |                      14. Load policy bindings (PG)                  |
    |                      15. Check model access                        |
    |                      16. Check token limits                        |
    |                      17. Check budget (PG aggregate)               |
    |                      18. Check rate limit (Redis INCR)             |
    |                       --- Policy Passed ---                         |
    |                      19. Resolve model alias to route (PG/cache)   |
    |                      20. Build execution plan (primary + fallback) |
    |                       --- Route Resolved ---                       |
    |                      21. Transform to provider format              |
    |                            |--- POST provider API ----------------->|
    |                            |                                        |
    |                            |<--- 200 JSON response -----------------|
    |                      22. Normalize provider response               |
    |                      23. Persist usage event (PG, async)           |
    |                      24. Emit metrics + traces                     |
    |                            |                                        |
    |<--- 200 JSON response -----|                                        |
```

### 5.2 Data Plane -- Streaming Request Lifecycle

```
Client SDK                  Gateway Service                          LLM Provider
    |                            |                                        |
    |--- POST (stream=true) ---->|                                        |
    |                            |                                        |
    |                   [Steps 5-20 same as non-streaming]                |
    |                            |                                        |
    |                      21. Transform to provider format              |
    |                            |--- POST provider API (stream) -------->|
    |                            |                                        |
    |<-- SSE: response.started --|<--- SSE chunk 1 ----------------------|
    |<-- SSE: response.delta ----|<--- SSE chunk 2 ----------------------|
    |<-- SSE: response.delta ----|<--- SSE chunk N ----------------------|
    |<-- SSE: response.completed-|<--- SSE [DONE] -----------------------|
    |                            |                                        |
    |                      22. Accumulate token counts from chunks        |
    |                      23. Persist usage event (PG, async)           |
    |                      24. Emit metrics + traces                     |
    |                            |                                        |
    |   --- Client Disconnect ---|                                        |
    |                      25. Detect TCP/SSE disconnect                  |
    |                            |--- Cancel upstream request ----------->|
    |                      26. Mark usage event as partial                |
```

### 5.3 Data Plane -- Retry & Fallback Flow

```
Gateway Service                Provider A          Provider B (fallback)
    |                              |                       |
    |--- Attempt 1 (Provider A) -->|                       |
    |<--- 5xx / timeout -----------|                       |
    |                              |                       |
    | [retry_count < max_retries]  |                       |
    |                              |                       |
    |--- Attempt 2 (Provider A) -->|                       |
    |<--- 5xx / timeout -----------|                       |
    |                              |                       |
    | [retries exhausted, try fallback]                    |
    |                              |                       |
    |--- Attempt 3 (Provider B) --|----------------------->|
    |<--- 200 -------------------|--------------------------|
    |                              |                       |
    | Record: retry_count=2, final_provider=B              |
```

### 5.4 Control Plane -- Admin Action Flow

```
Admin Console              Gateway Service (Admin API)           PostgreSQL
    |                              |                                  |
    |--- POST /admin/sa (JWT) ---->|                                  |
    |                              |                                  |
    |                         1. Validate JWT                        |
    |                         2. Validate request body               |
    |                         3. Check admin permissions              |
    |                              |--- INSERT service_account ------>|
    |                              |<--- RETURNING * -----------------|
    |                              |--- INSERT audit_event ---------->|
    |                              |<--- OK --------------------------|
    |                              |                                  |
    |<--- 201 Created -------------|                                  |
```

---

## 6. Deployment Architecture

### 6.1 Single-Node Deployment (Development / Small Scale)

Suitable for development, testing, and small production deployments (< 100 RPS).

```
+-------------------------------------------------------+
|                   Single Host / VM                      |
|                                                         |
|  +------------------+  +------------------+             |
|  | Gateway Service  |  | Admin Console    |             |
|  | (port 8080)      |  | (port 3000)      |             |
|  +--------+---------+  +--------+---------+             |
|           |                      |                      |
|  +--------v---------+  +--------v---------+             |
|  | PostgreSQL 17    |  | Valkey 8         |             |
|  | (port 5432)      |  | (port 6379)      |             |
|  +------------------+  +------------------+             |
+-------------------------------------------------------+
```

**Orchestration**: Docker Compose

```yaml
# docker-compose.yml (conceptual)
services:
  gateway:
    image: llmsmartgate/gateway:latest
    ports: ["8080:8080"]
    depends_on: [postgres, valkey]
  
  admin-console:
    image: llmsmartgate/admin-console:latest
    ports: ["3000:3000"]
  
  postgres:
    image: postgres:17-alpine
    volumes: [pgdata:/var/lib/postgresql/data]
  
  valkey:
    image: valkey/valkey:8-alpine
```

### 6.2 Multi-Node Deployment (Production / High Scale)

Suitable for production deployments requiring high availability and horizontal scaling.

```
                        +-------------------+
                        |  Load Balancer    |
                        |  (L7 / TLS term.) |
                        +--------+----------+
                                 |
                 +---------------+---------------+
                 |               |               |
          +------v------+ +-----v-------+ +-----v-------+
          | Gateway #1  | | Gateway #2  | | Gateway #N  |
          | (stateless) | | (stateless) | | (stateless) |
          +------+------+ +------+------+ +------+------+
                 |               |               |
          +------v---------------v---------------v------+
          |                Shared State                  |
          |                                              |
          |  +--------------------+  +-----------------+ |
          |  | PostgreSQL Cluster |  | Valkey Cluster  | |
          |  | Primary + Replica  |  | 3-node min.    | |
          |  +--------------------+  +-----------------+ |
          +----------------------------------------------+

          +----------------------------------------------+
          |           Observability Stack                 |
          |  +----------+ +----------+ +-------------+   |
          |  | Prometheus| | Jaeger / | | Grafana     |   |
          |  | (metrics) | | Tempo    | | (dashboards)|   |
          |  +----------+ | (traces) | +-------------+   |
          |               +----------+                   |
          +----------------------------------------------+
```

**Orchestration**: Kubernetes (Helm chart)

Key Kubernetes resources:
- `Deployment` for Gateway (replicas: 3+, HPA on CPU/RPS)
- `Deployment` for Admin Console (replicas: 2)
- `StatefulSet` for PostgreSQL (or managed PG service)
- `StatefulSet` for Valkey (or managed Redis/Valkey service)
- `Service` (ClusterIP) for internal routing
- `Ingress` / `Gateway API` for external TLS termination
- `ConfigMap` for gateway configuration
- `Secret` for provider API keys, DB credentials
- `PodDisruptionBudget` for zero-downtime rollouts
- `NetworkPolicy` for inter-pod network segmentation

---

## 7. Network Architecture & Security Zones

```
+------------------------------------------------------------------+
|  ZONE 1: PUBLIC (Internet)                                        |
|                                                                    |
|  [Client SDKs]  [Admin Browsers]                                   |
|       |               |                                            |
+-------v---------------v------------------------------------------+
|  ZONE 2: DMZ (Load Balancer / Reverse Proxy)                      |
|                                                                    |
|  [TLS Termination] [WAF] [DDoS Protection]                        |
|       |               |                                            |
+-------v---------------v------------------------------------------+
|  ZONE 3: APPLICATION (Gateway + Admin Console)                     |
|                                                                    |
|  [Gateway Pods]  [Admin Console Pods]                              |
|       |               |                                            |
|  Allowed egress:                                                   |
|    - Zone 4 (databases)                                           |
|    - Zone 5 (LLM providers)                                       |
|    - Zone 6 (observability)                                        |
|                                                                    |
+-------v----------------------------------------------------------+
|  ZONE 4: DATA (Databases)                                         |
|                                                                    |
|  [PostgreSQL]  [Valkey]                                            |
|                                                                    |
|  Allowed ingress: Zone 3 only                                     |
|  No egress to public internet                                     |
|                                                                    |
+------------------------------------------------------------------+
|  ZONE 5: EXTERNAL PROVIDERS (Egress Only)                         |
|                                                                    |
|  [OpenAI]  [Anthropic]  [Gemini]  [Azure]  [vLLM]                 |
|                                                                    |
|  Outbound HTTPS from Zone 3 only                                  |
|  Provider API keys stored in Zone 4 secrets                       |
+------------------------------------------------------------------+
|  ZONE 6: OBSERVABILITY (Monitoring)                               |
|                                                                    |
|  [Prometheus]  [Jaeger/Tempo]  [Grafana]  [Loki]                   |
|                                                                    |
|  Allowed ingress: Zone 3 (push), Zone 6 internal                  |
+------------------------------------------------------------------+
```

**Network Policies:**

| Rule | Source Zone | Destination Zone | Ports | Protocol |
|------|-----------|-----------------|-------|----------|
| Client to Gateway | Zone 1 | Zone 2 -> Zone 3 | 443 | HTTPS |
| Gateway to PostgreSQL | Zone 3 | Zone 4 | 5432 | TCP (TLS) |
| Gateway to Valkey | Zone 3 | Zone 4 | 6379 | TCP (TLS) |
| Gateway to Providers | Zone 3 | Zone 5 | 443 | HTTPS |
| Gateway to Observability | Zone 3 | Zone 6 | 4317/4318 | gRPC/HTTP (OTLP) |
| Inter-zone deny | Any | Any (unlisted) | * | DENY |

---

## 8. Scalability Strategy

### 8.1 Horizontal Scaling Plan

| Component | Scaling Axis | Strategy | Trigger |
|-----------|-------------|----------|---------|
| **Gateway Service** | Horizontal (pods) | Add replicas behind load balancer. Stateless design means any instance handles any request. | CPU > 70% or RPS > threshold per pod |
| **PostgreSQL** | Vertical + Read Replicas | Scale primary vertically for writes. Add streaming replicas for read-heavy admin queries (usage, audit). | Write latency > 10ms p99 or read queue depth |
| **Valkey** | Horizontal (cluster) | Valkey Cluster with hash-slot distribution. Nonce keys naturally distribute by service_account_id prefix. | Memory > 80% or ops/sec > single-node capacity |
| **Admin Console** | Horizontal (pods) | Static assets served via CDN. SSR pods scale independently. | Low priority -- admin traffic is minimal |

### 8.2 Bottleneck Analysis and Mitigation

| Bottleneck | Symptom | Mitigation |
|-----------|---------|------------|
| PostgreSQL writes (usage_events) | Insert latency spikes | 1. Batch inserts via channel (tokio::mpsc). 2. Time-range partitioning (monthly). 3. BRIN indexes on created_at. |
| Redis/Valkey nonce checks | Latency on auth path | Pipeline nonce SET + rate limit INCR in single round-trip. Use Valkey pipelining. |
| Provider API latency | High p99 tail latency | Timeout per provider (30s default). Fallback to secondary provider. Hedged requests (v2). |
| Connection pool exhaustion | 5xx spikes under load | Size pools based on load testing. Monitor checkout wait time. Alert on queue depth. |

### 8.3 Capacity Planning Guidelines

| Metric | Single Gateway Pod | Scaling Formula |
|--------|-------------------|-----------------|
| RPS (non-streaming) | ~5,000 RPS | pods = ceil(target_RPS / 4000) |
| Concurrent streams | ~2,000 | pods = ceil(target_streams / 1500) |
| PG connections per pod | 20 (pool size) | total_PG_connections = pods * 20 + 10 (admin) |
| Valkey connections per pod | 10 (pool size) | total_Valkey_connections = pods * 10 |

---

## 9. Disaster Recovery & High Availability

### 9.1 HA Architecture

| Component | HA Strategy | RTO | RPO |
|-----------|------------|-----|-----|
| Gateway Service | Multi-replica (3+) behind LB. Rolling deploys with PDB. Health checks on `/healthz`. | 0 (instant failover) | N/A (stateless) |
| PostgreSQL | Primary-Replica with streaming replication. Automatic failover via Patroni or cloud-managed. | < 30 seconds | 0 (synchronous replication) |
| Valkey | Valkey Cluster (3 masters, 3 replicas) or Sentinel mode. | < 10 seconds | ~1 second (async replication) |
| Admin Console | Multi-replica. Non-critical -- degraded mode acceptable. | < 60 seconds | N/A (stateless) |

### 9.2 Failure Mode Analysis

| Failure | Impact | Automatic Recovery | Manual Action |
|---------|--------|-------------------|---------------|
| Single gateway pod crash | None (LB routes to healthy pods) | Kubernetes restarts pod | None |
| PostgreSQL primary down | New writes fail. Reads may continue on replica. Gateway returns 503 for requests needing DB. | Patroni promotes replica | Verify data consistency |
| Valkey down | Nonce checks fail (fail-safe: deny request). Rate limiting unavailable. | Valkey Sentinel/Cluster promotes replica | Monitor for replay attacks during gap |
| LLM provider outage | Requests to that provider fail | Fallback chain activates | Monitor cost impact of fallback provider |
| DNS failure | Complete outage | N/A | Switch DNS or use IP-based routing |

### 9.3 Backup Strategy

| Data | Method | Frequency | Retention |
|------|--------|-----------|-----------|
| PostgreSQL (all tables) | pg_dump logical backup | Daily | 30 days |
| PostgreSQL (WAL) | Continuous WAL archival to object storage | Continuous | 7 days |
| Gateway configuration | Version-controlled (Git) | On change | Indefinite |
| Valkey | RDB snapshots | Every 15 minutes | 24 hours (ephemeral data only) |

---

## 10. Performance Architecture

### 10.1 Caching Strategy

| Cache Layer | Data | Storage | TTL | Invalidation |
|-------------|------|---------|-----|-------------|
| Route resolution | model_alias -> provider_route mappings | Valkey | 60 seconds | On route CRUD via Admin API (explicit DELETE key) |
| Service account + key | Active keys for signature verification | In-process (moka) | 30 seconds | On key revoke via Admin API (TTL expiry) |
| Policy bindings | Policies for authenticated principal | In-process (moka) | 30 seconds | On policy change (TTL expiry) |
| Provider health | Provider availability status | Valkey | 10 seconds | On health check result |
| Nonce dedup | Used nonces for replay protection | Valkey | 300 seconds (matches timestamp window) | Automatic TTL expiry |

**Cache hierarchy**: In-process (moka, ~1us) -> Valkey (~0.5ms) -> PostgreSQL (~2-5ms)

### 10.2 Connection Pooling

| Connection | Library | Min Pool | Max Pool | Idle Timeout | Max Lifetime |
|-----------|---------|----------|----------|-------------|-------------|
| PostgreSQL | sqlx built-in pool | 5 | 20 | 300s | 1800s |
| Valkey | deadpool-redis | 5 | 10 | 300s | 1800s |
| Upstream HTTP (providers) | reqwest (built-in pool) | -- | 100 per host | 90s | 300s |

### 10.3 Async I/O Architecture

The entire request path is fully asynchronous, built on Tokio's multi-threaded runtime:

1. **Request ingestion**: Axum + Hyper async accept
2. **Auth verification**: Async PG query for key lookup, async Redis for nonce check
3. **Policy evaluation**: Async PG query, async Redis for rate limit
4. **Provider call**: reqwest async HTTP client with timeout
5. **Streaming relay**: tokio::select! for concurrent upstream read + downstream write + disconnect detection
6. **Usage persistence**: Fire-and-forget via tokio::spawn onto a background task (with bounded channel backpressure)

**Tokio runtime configuration:**

```rust
// Production runtime configuration
tokio::runtime::Builder::new_multi_thread()
    .worker_threads(num_cpus::get())  // Match CPU cores
    .max_blocking_threads(64)          // For rare blocking ops
    .enable_all()
    .build()
```

### 10.4 Zero-Copy Streaming

For SSE streaming, the gateway uses `bytes::Bytes` with zero-copy forwarding:

1. Provider response chunks arrive as `Bytes` (reference-counted, no copy)
2. Gateway parses SSE frame boundaries without copying payload data
3. Normalized event is re-framed as SSE and forwarded to client
4. Token counting happens on the parsed event metadata, not by re-parsing the full payload

### 10.5 Performance Targets

| Metric | Target | Measurement Point |
|--------|--------|-------------------|
| Gateway overhead (non-streaming) | < 10ms p50, < 50ms p95, < 200ms p99 | request_in to provider_call_start |
| Gateway overhead (streaming TTFB) | < 15ms p95 | request_in to first_SSE_chunk_out |
| Auth verification | < 5ms p95 | auth_start to auth_complete |
| Policy evaluation | < 2ms p95 | policy_start to policy_complete |
| Route resolution (cached) | < 1ms p95 | route_start to route_complete |
| Usage persistence | < 5ms p95 (async, non-blocking) | usage_emit to usage_persisted |

---

# Part 2: Technology Standards

---

## 11. Technology Stack Decision Matrix

### 11.1 Backend Runtime & Framework

| Criterion | Axum 0.8 (Selected) | Actix-web 4.x | Rocket 0.5 | Warp 0.3 |
|-----------|---------------------|---------------|------------|----------|
| **Performance** | Near-identical to Actix-web (within 10-15%). Lower memory per connection. | Highest raw throughput. ~10-15% more RPS under extreme load. | Moderate. Macro-heavy compilation. | Comparable to Axum. |
| **Tokio integration** | Native -- built by the Tokio team. Direct tower/hyper integration. | Uses own actor runtime atop Tokio. Extra abstraction layer. | Tokio-based since 0.5 but less integrated. | Built on hyper, good Tokio integration. |
| **Middleware ecosystem** | Full tower + tower-http ecosystem (tracing, CORS, compression, timeout, request-id). | Own middleware system. Cannot reuse tower middleware directly. | Fairings (custom system). Limited ecosystem. | Filter-based composition. Unique API. |
| **Community & maintenance** | Backed by Tokio team. Active development. axum 0.8 released Jan 2025. | Strong community. Independent maintainers. | Slower release cadence. | Maintenance mode. |
| **Learning curve** | Moderate. Type-driven extractors. | Moderate. Actor model adds concepts. | Easy for simple cases. Macros hide complexity. | Steep. Filter combinators are abstract. |
| **SSE/streaming** | First-class via Sse<> type and axum::response::Sse. | Supported via HttpResponse::streaming. | Supported but less ergonomic. | Supported via warp::sse. |

**Decision**: **Axum 0.8** -- selected for native Tokio integration, tower middleware reuse, lower memory footprint, active maintenance by the Tokio team, and first-class SSE support. The 10-15% raw throughput advantage of Actix-web does not justify the ecosystem fragmentation of maintaining a separate middleware stack. Axum's memory efficiency is particularly valuable for container deployments where density matters.

### 11.2 Database Access

| Criterion | SQLx 0.8 (Selected) | Diesel 2.3 | SeaORM 2.0 |
|-----------|---------------------|------------|------------|
| **Async support** | Native async-first. Built for Tokio. | Requires diesel-async crate. Not native. | Async-first (built on SQLx). |
| **Compile-time checking** | Yes -- verifies SQL at compile time against live DB. | Yes -- via schema.rs code generation. | Partial -- via entity codegen. |
| **Connection pooling** | Built-in pool (no external dependency). | Requires r2d2 (sync) or deadpool (async). | Delegates to SQLx pool. |
| **Query style** | Raw SQL with compile-time verification. Full control. | DSL-based query builder. Type-safe but verbose. | ActiveRecord-style ORM. Convenient but abstracts. |
| **Migration support** | Built-in (sqlx migrate). | Built-in (diesel migration). | Built-in (sea-orm-migration). |
| **PostgreSQL features** | Full access to PG-specific types (JSONB, arrays, enums). | Good PG support. | Good PG support via SQLx. |

**Decision**: **SQLx 0.8** -- selected for native async-first design, built-in connection pooling (eliminating deadpool/bb8 dependency for PG), compile-time SQL verification, and direct SQL control needed for performance-critical queries (usage event inserts, rate limit aggregations). The gateway's queries are well-defined and benefit more from raw SQL performance than ORM convenience.

### 11.3 In-Memory Data Store

| Criterion | Valkey 8 (Selected) | Redis 7.4 | DragonflyDB | KeyDB |
|-----------|---------------------|-----------|-------------|-------|
| **License** | BSD-3 (permissive). Linux Foundation governed. | SSPL/RSALv2 (pre-8) or AGPLv3 (8+). Copyleft concerns for self-hosted. | BSL 1.1. Not fully open-source. | BSD-3. |
| **Compatibility** | 100% Redis protocol compatible. Drop-in replacement. | N/A (baseline). | 99%+ compatible. Some edge cases. | 99%+ compatible. |
| **Performance** | On par with Redis for most workloads. Leads in batched operations. | Baseline. Excellent single-threaded performance. | 10-25x throughput on multi-core. Best write throughput. | 2-5x throughput via multi-threading. |
| **Clustering** | Redis Cluster protocol. Sentinel support. | Redis Cluster. Sentinel. | Single-instance scales vertically (no clustering needed until very large). | Active-active replication. |
| **Community** | AWS, Google Cloud, Akamai backing. Rapidly growing. | Established but license concerns driving migration. | Growing commercial adoption. | Smaller community. |
| **Operational maturity** | Production at AWS ElastiCache, GCP Memorystore. | Decades of production use. | Newer. Less operational tooling. | Moderate. |

**Decision**: **Valkey 8** -- selected for BSD-3 licensing (no copyleft concerns for self-hosted distribution), 100% Redis protocol compatibility (zero code changes), strong cloud vendor backing (AWS, GCP, Akamai), and equivalent performance for our workload (nonce TTL, rate limit counters, health cache). DragonflyDB offers superior throughput but introduces compatibility risk and BSL licensing concerns. Redis 7.4 remains a supported fallback since the gateway uses standard Redis commands only.

### 11.4 Frontend Framework

| Criterion | SvelteKit 2 + Svelte 5 (Selected) | Next.js 15 | Nuxt 4 |
|-----------|-------------------------------------|------------|--------|
| **Bundle size** | Smallest. Svelte compiles to vanilla JS. 15-30% smaller than React. | Larger. React runtime overhead. | Larger. Vue runtime overhead. |
| **Performance** | Best Core Web Vitals. Surgical hydration. | Good with RSC. Heavier hydration. | Good. Similar to Next.js. |
| **State management** | Runes ($state, $derived, $effect). No external library needed. | useState/useReducer + external (Zustand/Jotai). | Composables + Pinia. |
| **UI library** | shadcn-svelte (copy-paste components). Tailwind CSS v4. | shadcn/ui (original). | Nuxt UI / PrimeVue. |
| **SSR/SSG** | Built-in. Adapter pattern for any deployment target. | Built-in. Vercel-optimized. | Built-in. |
| **Learning curve** | Low for admin consoles. HTML-first mental model. | Moderate. JSX + hooks complexity. | Moderate. |

**Decision**: **SvelteKit 2 with Svelte 5** -- selected for smallest bundle size (critical for admin console load time), built-in Runes-based state management (no external dependency), excellent TypeScript support, and shadcn-svelte providing high-quality accessible UI components. The admin console is an internal tool where developer velocity and bundle size outweigh React ecosystem breadth.

### 11.5 Python SDK HTTP Client

| Criterion | httpx (Selected) | requests + aiohttp | urllib3 |
|-----------|-------------------|-------------------|---------|
| **Sync + async** | Single library supports both. Same API surface. | Two separate libraries with different APIs. | Sync only. |
| **HTTP/2** | Built-in support. | requests: no. aiohttp: no. | No. |
| **Type hints** | Fully type-annotated. | requests: partial. aiohttp: good. | Partial. |
| **Streaming** | Async streaming iteration. | aiohttp: good. requests: basic. | Basic. |
| **Connection pooling** | Built-in for both sync and async. | Separate implementations. | Built-in (sync). |

**Decision**: **httpx** -- selected for unified sync/async API (simplifies SDK maintenance), HTTP/2 support, full type annotations, and excellent streaming support needed for SSE consumption.

---

## 12. Rust Crate Standards

### 12.1 Approved Crate Registry

All crates must be from this approved list. Adding a new crate requires a security review and team approval.

#### Web Framework & HTTP

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `axum` | 0.8.x | HTTP framework | Tokio-native, tower middleware, first-class SSE. See Section 11.1. |
| `axum-extra` | 0.12.x | Extended extractors | TypedHeader, Query, cookie support. |
| `tower` | 0.5.x | Middleware framework | Foundation for composable middleware layers. |
| `tower-http` | 0.6.x | HTTP middleware | CORS, compression, tracing, request-id, timeout. Battle-tested. |
| `hyper` | 1.x | HTTP implementation | Underlying HTTP engine. Pulled transitively by axum. |
| `reqwest` | 0.13.x | HTTP client | Outbound calls to LLM providers. Connection pooling, TLS, timeouts, compression. Rustls is the default TLS backend since 0.13. `query` and `form` are opt-in features. |

#### Serialization

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `serde` | 1.x | Serialization framework | Industry standard. Zero-cost abstractions via derive macros. |
| `serde_json` | 1.x | JSON serialization | Fast JSON parsing. Streaming deserializer for large payloads. |
| `serde_yaml` | 0.9.x | YAML config parsing | Configuration file support. |

#### Database Access

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `sqlx` | 0.8.x | PostgreSQL client | Async-first, compile-time SQL verification, built-in pool. See Section 11.2. |
| `sqlx` (feature: `postgres`, `runtime-tokio`, `tls-rustls`, `uuid`, `chrono`, `json`) | 0.8.x | Feature flags | Enable PG driver, Tokio runtime, TLS, UUID type, timestamp type, JSONB type. |

#### Redis / Valkey Client

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `redis` | 1.x | Valkey/Redis client | Async support, connection multiplexing, pipelining. Full Valkey compatibility confirmed in 1.0. Note: `FromRedisValue` trait uses owned values (changed from reference-based in 0.x). |
| `deadpool-redis` | 0.23.x | Connection pooling for Redis | Simple API, automatic recycling, integrates with Tokio. Aligned with redis 1.x. Preferred over bb8 for its minimal API surface. |

#### Cryptography

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `ed25519-dalek` | 2.x | Ed25519 signing/verification | Pure-Rust, audited, ~500k signatures/sec. The standard choice for Ed25519 in Rust. |
| `sha2` | 0.10.x | SHA-256 hashing | Body hash computation (X-Body-SHA256). Part of RustCrypto project. |
| `rand` | 0.8.x | Cryptographic RNG | Nonce generation. Uses OS entropy source. |
| `base64` | 0.22.x | Base64 encoding | Signature and key encoding. |
| `rustls` | 0.23.x | TLS implementation | Pure-Rust TLS. No OpenSSL dependency. Smaller attack surface. |

**Post-quantum readiness note**: Ed25519 is not post-quantum safe (vulnerable to Shor's algorithm). NIST post-quantum signature standardization (ML-DSA) is expected to finalize in 2026-2027. The architecture supports future hybrid mode (Ed25519 + ML-DSA) by:
1. The `algorithm` field in `service_account_keys` table already supports multiple algorithms.
2. The auth module's `verifier.rs` can dispatch by algorithm.
3. Key registration already accepts `algorithm` in the API contract.
4. When ML-DSA Rust crates reach production maturity, add `ml-dsa` as a supported algorithm alongside `ed25519`. No schema changes needed.

#### Logging & Tracing

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `tracing` | 0.1.x | Structured diagnostics | The Tokio ecosystem standard. Spans, events, structured fields. |
| `tracing-subscriber` | 0.3.x | Subscriber configuration | JSON formatting, EnvFilter, layered subscribers. |
| `tracing-opentelemetry` | 0.32.x | OpenTelemetry bridge | Bridges tracing spans to OTLP. Minimal overhead via batch export. Follows OTel version + 1 convention. |
| `opentelemetry` | 0.31.x | OpenTelemetry API | Trace context propagation, metric API. |
| `opentelemetry-otlp` | 0.31.x | OTLP exporter | Push traces and metrics to OTLP-compatible backends. |
| `opentelemetry_sdk` | 0.31.x | OTel SDK | Batch span processor, metric reader. |
| `metrics` | 0.24.x | Metrics facade | Lightweight counters/gauges/histograms. |
| `metrics-exporter-prometheus` | 0.16.x | Prometheus exporter | Expose /metrics endpoint for Prometheus scraping. |

#### Testing

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `tokio` (feature: `test`) | 1.x | Async test runtime | `#[tokio::test]` macro for async unit tests. |
| `wiremock` | 0.6.x | HTTP mock server | Mock LLM provider responses in integration tests. |
| `sqlx` (feature: `test`) | 0.8.x | Database test utilities | Test transactions that auto-rollback. |
| `assert_json_diff` | 2.x | JSON assertion | Readable JSON comparison in tests. |
| `criterion` | 0.5.x | Benchmarking | Statistically rigorous performance benchmarks. |
| `proptest` | 1.x | Property-based testing | Fuzz policy engine, auth verification edge cases. |

#### Error Handling

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `thiserror` | 1.x | Library error types | Derive macro for structured error enums with Display. Used in all internal modules. |
| `anyhow` | 1.x | Application error handling | Context-rich error chains in main/CLI code. NOT used in library modules. |

**Error handling rule**: Library code (anything under `src/`) uses `thiserror` with explicit error enums. Only `main.rs`, CLI entry points, and test code may use `anyhow`.

#### Utilities

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `tokio` | 1.x (LTS) | Async runtime | The Rust async runtime standard. Multi-threaded work-stealing scheduler. |
| `uuid` | 1.x | UUID generation | v4 random UUIDs for entity IDs. v7 time-sorted UUIDs for usage_events (better index locality). |
| `chrono` | 0.4.x | Date/time handling | Timezone-aware timestamps. PostgreSQL TIMESTAMPTZ interop. |
| `bytes` | 1.x | Byte buffer | Zero-copy streaming. Reference-counted byte slices. |
| `moka` | 0.12.x | In-process cache | Concurrent, bounded, TTL-based cache. Used for hot-path caching (keys, policies). |
| `dotenvy` | 0.15.x | Environment loading | .env file support for local development. |
| `config` | 0.15.x | Configuration | Layered config from files, env vars, defaults. |
| `num_cpus` | 1.x | CPU detection | Size Tokio runtime worker threads. |

### 12.2 Banned Crates

| Crate | Reason |
|-------|--------|
| `openssl` / `native-tls` | Prefer `rustls` for pure-Rust TLS. Eliminates C dependency and associated CVE surface. |
| `async-std` | Discontinued as of March 2025. Tokio is the standard. |
| `actix-rt` | Avoid Actix runtime. All async code must run on Tokio. |
| `chrono-tz` | Use `chrono` with UTC only. No timezone database dependency. |

---

## 13. SvelteKit Standards

### 13.1 Framework Version Requirements

| Technology | Version | Notes |
|-----------|---------|-------|
| Svelte | 5.x | Runes-based reactivity. No legacy `$:` reactive statements. |
| SvelteKit | 2.x | Adapter-neutral deployment. |
| TypeScript | 6.x | Last JS-based release. Defaults: `strict: true`, `target: es2025`, ESM-first. TS 7.0 (Go-native rewrite, 10x faster) in dev preview -- plan for future migration; use `--stableTypeOrdering` flag to prepare. |
| Tailwind CSS | 4.x | Utility-first CSS. CSS-first configuration (no tailwind.config.js). |
| shadcn-svelte | latest | Generated components in `$lib/components/ui/`. |
| bits-ui | latest | Headless primitives underlying shadcn-svelte. |
| Vite | 8.x | Rolldown Rust-based bundler (10-30x faster builds). Replaces esbuild+Rollup dual architecture. `build.rollupOptions` renamed to `build.rolldownOptions`. |
| ESLint | 10.x | Flat config mandatory (`.eslintrc` removed). `@typescript-eslint` + `eslint-plugin-svelte`. |
| Prettier | 3.8.x | Default + `prettier-plugin-svelte`. |
| Package manager | pnpm `[RECOMMENDED]` | Strict deps, disk efficiency, monorepo-native. Use `pnpm.overrides` for `typescript: "^6.0.0"` until SvelteKit widens its `^5.3.3` peer dependency. |

### 13.2 Component Patterns

**State management with Runes (Svelte 5):**

```svelte
<script lang="ts">
  // Use $state for reactive variables
  let count = $state(0);
  
  // Use $derived for computed values
  let doubled = $derived(count * 2);
  
  // Use $effect for side effects
  $effect(() => {
    console.log(`Count changed to ${count}`);
  });
</script>
```

**Reactive classes for shared state (replaces Svelte stores):**

```typescript
// src/lib/state/tenant-state.svelte.ts
export class TenantState {
  tenants = $state<Tenant[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  
  selectedCount = $derived(this.tenants.filter(t => t.selected).length);
  
  async load() {
    this.loading = true;
    try {
      this.tenants = await api.tenants.list();
    } catch (e) {
      this.error = e.message;
    } finally {
      this.loading = false;
    }
  }
}

export const tenantState = new TenantState();
```

**Component structure rules:**

1. One component per file. File name matches component name in PascalCase.
2. All props use the `$props()` rune with TypeScript types.
3. Event handlers use callback props, not Svelte events.
4. Components must be accessible (ARIA attributes, keyboard navigation).
5. Use `use:action` for DOM behavior (focus traps, click-outside).

### 13.3 Project Structure

```
src/
  lib/
    components/
      ui/           # shadcn-svelte generated components (DO NOT manually edit)
      app/          # Application-specific components
        DataTable.svelte
        MetricsChart.svelte
        PolicyEditor.svelte
    state/          # Reactive classes (.svelte.ts files)
      tenant-state.svelte.ts
      auth-state.svelte.ts
    api/            # API client layer
      client.ts     # Base fetch wrapper with auth
      tenants.ts    # Tenant API methods
      policies.ts   # Policy API methods
    types/          # TypeScript type definitions
      api.ts        # API request/response types
      models.ts     # Domain model types
    utils/          # Pure utility functions
  routes/
    +layout.svelte       # Root layout (sidebar, nav)
    +layout.server.ts    # Root server load (auth check)
    dashboard/
      +page.svelte
      +page.server.ts
    tenants/
      +page.svelte
      +page.server.ts
      [id]/
        +page.svelte
        +page.server.ts
```

### 13.4 API Integration Pattern

```typescript
// src/lib/api/client.ts
const BASE_URL = import.meta.env.VITE_API_BASE_URL;

async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${getToken()}`,
      ...options?.headers,
    },
  });
  
  if (!response.ok) {
    const error = await response.json();
    throw new ApiError(response.status, error);
  }
  
  return response.json();
}
```

**Rules:**
1. All API calls go through `+page.server.ts` (server-side) to keep API tokens secure.
2. Client-side fetches use SvelteKit's `fetch` (which handles cookies and CSRF).
3. Error responses are caught and rendered via SvelteKit error boundaries.
4. Pagination state is stored in URL search params, not component state.

### 13.5 Testing Standards

| Layer | Tool | Version | Scope |
|-------|------|---------|-------|
| Unit (logic) | Vitest | 4.x (required for Vite 8) | Reactive classes, utility functions, API client transforms. |
| Component | Vitest + `vitest-browser-svelte` + `@vitest/browser` `[RECOMMENDED]` | 4.x | Browser-based component testing with Playwright provider. Replaces `@testing-library/svelte` + jsdom for more accurate DOM behavior. |
| E2E | Playwright | >=1.57 | Critical user flows: login, create service account, view usage. 1.57+ uses Chrome for Testing builds. |

---

## 14. Python SDK Standards

### 14.1 Technology Choices

| Technology | Version | Purpose |
|-----------|---------|---------|
| Python | >= 3.12 | Minimum supported version. 3.10 EOL Oct 2026, 3.11 EOL Oct 2027. 3.12 provides f-string improvements, `type` statement, and EOL Oct 2028. |
| uv | 0.11.x | Project manager, package installer, build tool, publisher. Replaces pip, virtualenv, pyenv, pipx. 10-100x faster than pip. |
| httpx | 0.28.x | HTTP client (sync + async). |
| pydantic | 2.x | Request/response models. Validation. Serialization. |
| ed25519 (PyNaCl or `cryptography`) | latest | Ed25519 signing. PyNaCl preferred for speed; `cryptography` as fallback. |
| pytest | 8.x | Testing framework. Run via `uv run pytest`. |
| pytest-asyncio | 0.24.x | Async test support. |
| mypy | 1.x | Static type checking. |
| ruff | 0.15.x | Linting and formatting. Includes 2026 style guide. |

### 14.2 SDK API Design

```python
# Synchronous client
from llmsmartgate import GatewayClient

client = GatewayClient(
    base_url="https://gateway.example.com",
    service_account_id="sa-abc123",
    key_id="key-xyz789",
    private_key_path="/path/to/private.pem",
)

# Non-streaming
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello"}],
)

# Streaming
for event in client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello"}],
    stream=True,
):
    print(event.choices[0].delta.content, end="")

# Async client
from llmsmartgate import AsyncGatewayClient

async_client = AsyncGatewayClient(...)
response = await async_client.chat.completions.create(...)
```

### 14.3 SDK Architecture Rules

1. **No global state.** All configuration lives on the client instance.
2. **Thread-safe.** The sync client must be safe to share across threads.
3. **Resource management.** Clients implement context manager protocol (`__enter__`/`__exit__`, `__aenter__`/`__aexit__`).
4. **Retry logic.** Configurable retry with exponential backoff for 5xx and timeout errors. Default: 3 retries.
5. **Timeout defaults.** Connect: 10s. Read: 60s. Streaming read: 300s.
6. **Type stubs.** Full `py.typed` marker and inline type annotations. Compatible with mypy strict mode.
7. **Minimal dependencies.** Only httpx, pydantic, and a crypto library. No other transitive dependencies.

### 14.4 Package Distribution

| Item | Standard |
|------|----------|
| Package name | `llmsmartgate` |
| Project manager | `uv` (all-in-one: install, resolve, venv, build, publish) |
| Build backend | `uv_build` (10-35x faster than hatchling; pure-Python only). Fallback to Hatchling if build hooks or VCS versioning needed. |
| Lockfile | `uv.lock` (cross-platform, committed to git) |
| Distribution | PyPI (public or private registry). Publish via `uv publish`. Supports Trusted Publishing (GitHub Actions OIDC). |
| Versioning | Semantic versioning (SemVer) |
| Changelog | CHANGELOG.md following Keep a Changelog format |

**pyproject.toml example:**

```toml
[build-system]
requires = ["uv_build>=0.11.2,<0.12"]
build-backend = "uv_build"

[project]
name = "llmsmartgate"
version = "0.1.0"
description = "Python SDK for LLMSmartGate"
requires-python = ">=3.12"
dependencies = [
    "httpx>=0.28",
    "pydantic>=2.10",
    "PyNaCl>=1.6",
]

[dependency-groups]
dev = ["pytest>=8.0", "pytest-asyncio>=0.24", "pytest-cov", "mypy>=1.0", "ruff>=0.15"]

[tool.pytest.ini_options]
testpaths = ["tests"]
asyncio_mode = "auto"

[tool.ruff]
target-version = "py312"

[tool.mypy]
strict = true
python_version = "3.12"
```

**Standard workflow:**

```bash
uv sync                     # Install dependencies + create venv
uv run pytest               # Run tests
uv run mypy src/            # Type check
uv run ruff check src/      # Lint
uv build                    # Build sdist + wheel
uv publish                  # Publish to PyPI
```

---

## 15. Database Standards

### 15.1 Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Table names | snake_case, plural | `service_accounts`, `usage_events` |
| Column names | snake_case | `service_account_id`, `created_at` |
| Primary keys | `id` (UUID) | `id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| Foreign keys | `{referenced_table_singular}_id` | `tenant_id`, `policy_id` |
| Indexes | `idx_{table}_{column(s)}` | `idx_usage_events_created_at` |
| Unique constraints | `uq_{table}_{column(s)}` | `uq_service_accounts_tenant_slug` |
| Enums | `{domain}_status` or `{domain}_type` | `tenant_status`, `budget_period_type` |
| JSONB columns | `{name}_json` suffix | `allowed_models_json`, `metadata_json` |
| Timestamps | `{action}_at` with TIMESTAMPTZ | `created_at`, `updated_at`, `revoked_at` |

### 15.2 UUID Strategy

| Table Category | UUID Version | Rationale |
|---------------|-------------|-----------|
| Control plane entities (tenants, service_accounts, policies, routes) | UUIDv4 (random) | No ordering requirement. Unpredictable IDs for public-facing identifiers. |
| Append-only events (usage_events, audit_events) | UUIDv7 (time-sorted) | Time-sorted for better B-tree index locality on append-heavy tables. Monotonically increasing prefix reduces page splits. |

### 15.3 Indexing Strategy

**General rules:**
1. Every foreign key gets an index (PostgreSQL does not auto-create FK indexes).
2. Columns used in WHERE, ORDER BY, or JOIN get indexes.
3. Avoid over-indexing -- each index adds write overhead.
4. Use BRIN indexes for append-only tables with time-ordered data.
5. Use partial indexes where appropriate (e.g., `WHERE status = 'active'`).

**Table-specific indexes:**

```sql
-- usage_events: High-volume append-only table
-- Use BRIN for time-range scans (compact, append-optimized)
CREATE INDEX idx_usage_events_created_at_brin 
    ON usage_events USING BRIN (created_at);

-- B-tree for exact lookups and filtered queries
CREATE INDEX idx_usage_events_tenant_sa 
    ON usage_events (tenant_id, service_account_id);

-- Partial index for active service account key lookups (hot path)
CREATE INDEX idx_sak_active_key_lookup 
    ON service_account_keys (key_id) 
    WHERE status = 'active';

-- Composite index for route resolution (hot path)
CREATE INDEX idx_provider_routes_alias_lookup 
    ON provider_routes (model_alias, enabled, priority) 
    WHERE enabled = true;
```

### 15.4 Partitioning Strategy

The `usage_events` table is the highest-volume table and must be partitioned:

```sql
-- Monthly range partitioning on created_at
CREATE TABLE usage_events (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    request_id TEXT NOT NULL,
    -- ... other columns ...
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
) PARTITION BY RANGE (created_at);

-- Create partitions for each month
CREATE TABLE usage_events_2026_04 
    PARTITION OF usage_events 
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');

-- Automate partition creation via pg_partman or a migration script
```

**Partition management:**
- Create partitions 3 months ahead.
- Detach and archive partitions older than the retention period (configurable, default: 12 months).
- Drop archived partitions to reclaim space (no VACUUM needed).

### 15.5 Migration Strategy

| Item | Standard |
|------|----------|
| Tool | `sqlx migrate` (integrated with SQLx) |
| File naming | `{timestamp}_{description}.sql` (e.g., `20260401120000_create_tenants.sql`) |
| Direction | Forward-only migrations. No down migrations. Rollback via compensating migrations. |
| Idempotency | Use `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS` where possible. |
| Deployment | Migrations run automatically on application startup (gated by advisory lock). |
| Review | Every migration must be reviewed for: index impact, lock duration, data migration safety. |
| Testing | Migrations tested against a clean database in CI. |

### 15.6 PostgreSQL Tuning Parameters (Production)

```ini
# Connection handling
max_connections = 200
# Reserve for superuser connections
superuser_reserved_connections = 3

# Memory
shared_buffers = '4GB'          # 25% of RAM (for 16GB host)
effective_cache_size = '12GB'   # 75% of RAM
work_mem = '64MB'               # Per-sort/hash operation
maintenance_work_mem = '512MB'  # For VACUUM, CREATE INDEX

# WAL (optimized for write throughput)
wal_buffers = '64MB'
wal_level = 'replica'
max_wal_size = '4GB'
min_wal_size = '1GB'
checkpoint_completion_target = 0.9

# Autovacuum (tuned for high-insert tables)
autovacuum_max_workers = 4
autovacuum_naptime = '30s'
autovacuum_vacuum_insert_threshold = 10000
autovacuum_vacuum_insert_scale_factor = 0.01

# Query planner
random_page_cost = 1.1          # For SSD storage
effective_io_concurrency = 200  # For SSD storage
```

---

## 16. API Design Standards

### 16.1 API Versioning

| Item | Standard |
|------|----------|
| Versioning scheme | URI path prefix: `/v1/`, `/v2/` |
| Data Plane API | `/v1/chat/completions`, `/v1/responses`, `/v1/embeddings`, `/v1/models` |
| Admin API | `/admin/v1/tenants`, `/admin/v1/service-accounts`, etc. |
| Breaking change policy | New major version for breaking changes. Old version supported for 12 months. |

### 16.2 Pagination

Admin API list endpoints use cursor-based pagination:

```json
// Request
GET /admin/v1/usage?cursor=eyJ0IjoiMjAyNi0wNC0wMSJ9&limit=50

// Response
{
  "data": [...],
  "pagination": {
    "next_cursor": "eyJ0IjoiMjAyNi0wMy0zMSJ9",
    "has_more": true,
    "limit": 50
  }
}
```

**Rules:**
- Default page size: 50.
- Maximum page size: 200.
- Cursor is an opaque base64-encoded token (server-controlled).
- No offset-based pagination (poor performance on large tables).

### 16.3 Error Response Format

All errors follow a consistent structure:

```json
{
  "error": {
    "code": "policy_denied",
    "message": "Model 'gpt-4o' is not allowed by policy 'restricted-models'.",
    "type": "forbidden",
    "request_id": "req_01HXYZ...",
    "details": {
      "policy_id": "pol_abc123",
      "denied_model": "gpt-4o"
    }
  }
}
```

**Standard error codes:**

| HTTP Status | Error Code | When |
|------------|-----------|------|
| 400 | `bad_request` | Malformed JSON, missing required fields |
| 400 | `invalid_parameter` | Field validation failure |
| 401 | `unauthorized` | Missing or invalid signature |
| 401 | `timestamp_expired` | Timestamp outside allowed skew window |
| 401 | `nonce_replay` | Nonce already used |
| 401 | `key_revoked` | Signing key has been revoked |
| 403 | `policy_denied` | Policy engine rejected the request |
| 403 | `budget_exceeded` | Budget limit reached |
| 404 | `not_found` | Resource does not exist |
| 409 | `conflict` | Duplicate resource (e.g., duplicate slug) |
| 422 | `unprocessable_entity` | Valid JSON but semantically invalid |
| 429 | `rate_limited` | Rate limit exceeded |
| 500 | `internal_error` | Unexpected server error |
| 502 | `provider_error` | LLM provider returned an error |
| 503 | `service_unavailable` | Gateway is temporarily unavailable |
| 504 | `provider_timeout` | LLM provider did not respond in time |

### 16.4 Rate Limit Headers

All responses include rate limit information:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1711929600
X-RateLimit-Policy: "1000;w=60"
Retry-After: 30  # Only on 429 responses
```

### 16.5 Request ID & Tracing Headers

```http
# Response headers (always present)
X-Request-Id: req_01HXYZ...
X-Trace-Id: 4bf92f3577b34da6a3ce929d0e0e4736

# Client can pass trace context for end-to-end tracing
Traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01
```

---

## 17. Security Standards

### 17.1 Transport Layer Security

| Requirement | Standard |
|------------|----------|
| TLS version | TLS 1.3 minimum. TLS 1.2 with approved cipher suites as fallback. |
| Certificate management | Automated via ACME (Let's Encrypt) or organization PKI. |
| TLS termination | At the load balancer/reverse proxy. Internal traffic between gateway and databases uses TLS. |
| 0-RTT | Disabled in v1 (prevents replay attacks at TLS layer). |
| mTLS (v2) | Optional for enterprise deployments. Client certificates verified at the gateway. |

### 17.2 Key Management

| Item | Standard |
|------|----------|
| Key algorithm | Ed25519 (current). ML-DSA-65 hybrid mode (future, when NIST standard finalized). |
| Key storage | Public keys stored in PostgreSQL (`service_account_keys.public_key_pem`). Private keys NEVER touch the gateway. |
| Key rotation | Supported via multi-key per service account. Overlap period allows gradual transition. |
| Key revocation | Immediate via `POST /admin/.../keys/{key_id}/revoke`. Revoked keys rejected on next request. |
| Key fingerprint | SHA-256 hash of the DER-encoded public key. Used for key identification in logs. |
| Key expiry | Optional `expires_at` field. Gateway rejects requests signed with expired keys. |

### 17.3 Secrets Management

| Secret Type | Storage | Access |
|------------|---------|--------|
| LLM provider API keys | Kubernetes Secrets (encrypted at rest) or external vault (HashiCorp Vault, AWS Secrets Manager) | Mounted as environment variables. Never in config files or source code. |
| PostgreSQL credentials | Kubernetes Secrets or vault | Connection string assembled from individual env vars. |
| Valkey password | Kubernetes Secrets or vault | Passed via `VALKEY_URL` env var. |
| Admin JWT signing key | Kubernetes Secrets or vault | Loaded once at startup. Rotatable via restart. |
| TLS certificates | Kubernetes TLS Secrets or cert-manager | Mounted as volume. Auto-renewed. |

**Rules:**
1. No secrets in source code, config files, Docker images, or logs.
2. Secrets are injected via environment variables or mounted volumes.
3. All secrets encrypted at rest in the secrets backend.
4. Secret rotation does not require gateway restart (except JWT signing key).
5. `cargo audit` CI check for known vulnerabilities in dependencies.

### 17.4 CORS Policy

```rust
// CORS configuration for Admin API
CorsLayer::new()
    .allow_origin(AllowOrigin::list([
        "https://admin.example.com".parse().unwrap(),
    ]))
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([
        header::CONTENT_TYPE,
        header::AUTHORIZATION,
    ])
    .max_age(Duration::from_secs(3600))
```

**Rules:**
- Data Plane API: No CORS (server-to-server only, called from SDKs not browsers).
- Admin API: Strict CORS with explicit origin allowlist. No wildcard (`*`) origins.

### 17.5 Request Security

| Protection | Implementation |
|-----------|---------------|
| **Replay protection** | Nonce uniqueness check in Valkey. TTL matches timestamp window (300s). |
| **Timestamp validation** | Request must arrive within +/- 300 seconds of `X-Timestamp`. |
| **Body integrity** | `X-Body-SHA256` verified against actual body hash. |
| **Signature verification** | Ed25519 signature over canonical string (method + path + timestamp + nonce + body_hash). |
| **Request size limit** | Maximum body size: 10MB (configurable). Enforced at Axum layer. |
| **Header injection** | All security-relevant headers are validated and sanitized. |
| **SQL injection** | Parameterized queries only (SQLx compile-time verification). No string concatenation in SQL. |
| **Path traversal** | Axum's router handles path normalization. No filesystem access from user input. |

### 17.6 Supply Chain Security

| Tool | Purpose | CI Integration |
|------|---------|---------------|
| `cargo audit` | Check dependencies against RustSec Advisory Database | Runs on every PR and daily scheduled CI |
| `cargo deny` | License compliance, banned crates, duplicate detection | Runs on every PR |
| `cargo auditable` | Embed dependency info in binary for runtime SBOM | Build pipeline flag |
| `cargo sbom` | Generate CycloneDX SBOM for release artifacts | Release pipeline |
| `trivy` | Container image vulnerability scanning. **Pin to v0.69.3** -- v0.69.4-0.69.6 were compromised on 2026-03-19. | Docker build pipeline. Pin action to commit SHA. |
| `grype` | Secondary container/SBOM vulnerability scanner `[RECOMMENDED]` | Docker build pipeline. Reduces false positives vs trivy alone. |
| Renovate | Automated dependency update PRs `[RECOMMENDED]` | GitHub Actions. 90+ package managers. Native grouping (1 PR vs 20). Handles Rust + Node + Python. Preferred over Dependabot for polyglot repos. |

**Supply chain hardening rules:**
1. `[MANDATORY]` Pin all GitHub Actions to **commit SHAs**, not tags (tags are mutable and can be hijacked).
2. `[MANDATORY]` Verify checksums for all security tooling downloaded in CI.
3. `[RECOMMENDED]` Use GitHub Actions native egress firewall for hosted runners (Layer 7, immutable).

---

## 18. Observability Standards

### 18.1 Structured Logging

**Format**: JSON, one object per line.

```json
{
  "timestamp": "2026-04-01T10:30:00.123456Z",
  "level": "INFO",
  "target": "llmsmartgate::auth::verifier",
  "message": "Signature verified",
  "span": {
    "request_id": "req_01HXYZ",
    "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
    "span_id": "00f067aa0ba902b7"
  },
  "fields": {
    "service_account_id": "sa-abc123",
    "key_id": "key-xyz789",
    "duration_ms": 2.3
  }
}
```

**Rules:**
1. All log entries include `request_id` and `trace_id` from the tracing span context.
2. Log levels follow this policy:
   - `ERROR`: Unrecoverable failures requiring investigation. Pages on-call in production.
   - `WARN`: Recoverable issues (retry succeeded, rate limit close to threshold).
   - `INFO`: Request lifecycle events (request received, response sent, provider called).
   - `DEBUG`: Detailed internal state (policy evaluation steps, cache hits/misses).
   - `TRACE`: Very verbose (full request/response bodies -- only in development).
3. **Never log**: Private keys, provider API keys, full request/response bodies (at INFO or above), PII.
4. **Always log**: request_id, trace_id, service_account_id, model_alias, provider, status_code, latency_ms.

### 18.2 Metrics Naming Convention

**Format**: `{namespace}_{subsystem}_{name}_{unit}`

**Namespace**: `llmsmartgate`

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `llmsmartgate_http_requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests received |
| `llmsmartgate_http_request_duration_seconds` | Histogram | `method`, `path`, `status` | Request duration (full lifecycle) |
| `llmsmartgate_auth_verifications_total` | Counter | `result` (success/failure/expired/replay) | Auth verification attempts |
| `llmsmartgate_auth_verification_duration_seconds` | Histogram | -- | Time to verify signature |
| `llmsmartgate_policy_evaluations_total` | Counter | `result` (allow/deny), `reason` | Policy evaluation outcomes |
| `llmsmartgate_provider_requests_total` | Counter | `provider`, `model`, `status` | Requests sent to LLM providers |
| `llmsmartgate_provider_request_duration_seconds` | Histogram | `provider`, `model` | Provider call duration |
| `llmsmartgate_provider_retries_total` | Counter | `provider`, `model` | Retry attempts |
| `llmsmartgate_provider_fallbacks_total` | Counter | `from_provider`, `to_provider` | Fallback activations |
| `llmsmartgate_tokens_total` | Counter | `type` (prompt/completion), `provider`, `model` | Token consumption |
| `llmsmartgate_estimated_cost_dollars` | Counter | `provider`, `model`, `tenant_id` | Estimated cost in USD |
| `llmsmartgate_streams_active` | Gauge | -- | Currently active streaming connections |
| `llmsmartgate_ratelimit_rejections_total` | Counter | `scope` (tenant/sa/global) | Rate limit 429 responses |
| `llmsmartgate_db_pool_connections` | Gauge | `pool` (pg/valkey), `state` (active/idle) | Connection pool utilization |

### 18.3 Distributed Tracing

**Trace context propagation**: W3C Trace Context (`traceparent` / `tracestate` headers).

**Span hierarchy:**

```
gateway.request (root span)
  |-- attributes: request_id, method, path, service_account_id, tenant_id
  |
  +-- auth.verify
  |     |-- attributes: key_id, algorithm
  |     +-- auth.nonce_check (Redis call)
  |     +-- auth.key_lookup (PG query)
  |     +-- auth.signature_verify (CPU-bound)
  |
  +-- policy.evaluate
  |     |-- attributes: policy_id, result (allow/deny)
  |     +-- policy.budget_check (PG query)
  |     +-- policy.rate_limit_check (Redis call)
  |
  +-- routing.resolve
  |     |-- attributes: model_alias, selected_provider, fallback_count
  |     +-- routing.cache_lookup (Redis/moka)
  |
  +-- provider.call
  |     |-- attributes: provider, model, attempt, is_streaming
  |     +-- provider.request_transform
  |     +-- provider.http_call (reqwest span)
  |     +-- provider.response_transform
  |
  +-- usage.persist
        |-- attributes: tokens_prompt, tokens_completion, cost, latency_ms
```

**Export configuration:**
- Protocol: OTLP gRPC (port 4317)
- Batch processor: max_export_batch_size=512, scheduled_delay=5s
- Sampling: Head-based sampling. 100% in development. 10% in production (configurable). Always sample errors.

### 18.4 Health Check Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /healthz` | Kubernetes liveness probe. Gateway process is running. | `200 {"status": "ok"}` |
| `GET /readyz` | Kubernetes readiness probe. Gateway can serve requests (DB and Redis reachable). | `200 {"status": "ready", "checks": {...}}` or `503` |
| `GET /metrics` | Prometheus metrics scrape endpoint. | Prometheus text format |

---

## 19. Code Quality Standards

### 19.1 Rust Code Quality

| Tool | Purpose | Configuration |
|------|---------|--------------|
| `rustfmt` | Code formatting | Default settings. `edition = "2024"`, `style_edition = "2024"`. Run via `cargo fmt`. |
| `clippy` | Linting | `#![deny(clippy::all)]`, `#![warn(clippy::pedantic)]`. Selected `pedantic` lints may be `allow`ed with justification. |
| `cargo test` | Unit + integration tests | All tests must pass. No `#[ignore]` without tracking issue. |
| `cargo-llvm-cov` | Code coverage `[MANDATORY]` | LLVM source-based instrumentation. Cross-platform (Linux/macOS/Windows), region-level accuracy. Minimum 80% line coverage for core modules (auth, policy, routing). |
| `cargo-nextest` | Test runner `[RECOMMENDED]` | Up to 3x faster than `cargo test`. Flaky test detection, JUnit/JSON output. Drop-in replacement. |
| `cargo doc` | Documentation | All public APIs must have doc comments. `#![deny(missing_docs)]` on library crate. |
| `cargo audit` | Security audit | Zero known vulnerabilities. Runs in CI. |
| `cargo deny` | Dependency governance | License check, duplicate crate detection, banned crate enforcement. |

### 19.2 Rust Code Conventions

1. **Edition & MSRV**: Rust edition 2024 `[MANDATORY]`. Minimum supported Rust version (MSRV): `rust-version = "1.85"` in Cargo.toml. Edition 2024 provides async closures, improved scoping rules, and resolver "2" by default.
2. **Error handling**: Use `Result<T, E>` everywhere. No `.unwrap()` in production code (except static initialization with compile-time guarantees). Use `.expect("reason")` only when panic is truly unreachable.
3. **Async**: All I/O operations are async. No `.block_on()` outside of `main()`.
4. **Cloning**: Minimize cloning. Use `Arc<T>` for shared ownership. Use `&str` over `String` in function parameters.
5. **Unsafe**: No `unsafe` code without a safety comment and team review. Prefer safe abstractions.
6. **Module structure**: One module per file. `mod.rs` only for re-exports.
7. **Testing**: Each module has a `#[cfg(test)] mod tests` section for unit tests. Integration tests in `tests/` directory.

### 19.3 TypeScript / SvelteKit Code Quality

| Tool | Purpose | Configuration |
|------|---------|--------------|
| `eslint` 10.x | Linting | Flat config only (`eslint.config.js`). `@typescript-eslint/recommended` + `eslint-plugin-svelte`. No `.eslintrc` (removed in v10). |
| `prettier` 3.8.x | Formatting | Default + `prettier-plugin-svelte`. |
| `svelte-check` 4.x | Type checking | Run in CI. Zero errors. Compatible with TypeScript 6.x. |
| `vitest` 4.x | Unit + component tests | Required for Vite 8. Minimum 70% coverage for API client and state modules. |
| `playwright` >=1.57 | E2E tests | Critical user flows. Run on staging before release. |

### 19.4 Python SDK Code Quality

| Tool | Purpose | Configuration |
|------|---------|--------------|
| `uv` 0.11.x | Project management | All commands run via `uv run`. Manages venv, deps, lockfile. |
| `ruff` 0.15.x | Linting + formatting | Replaces flake8, isort, black. Fast and comprehensive. Includes 2026 style guide. Run via `uv run ruff`. |
| `mypy` | Static type checking | Strict mode. All public APIs fully typed. Run via `uv run mypy`. |
| `pytest` | Testing | 90%+ coverage for signing logic and client methods. Run via `uv run pytest`. |
| `pytest-asyncio` | Async tests | Test both sync and async client paths. |

### 19.5 CI Gate Requirements

All of the following must pass before a PR can be merged:

| Gate | Applies To | Blocking |
|------|-----------|----------|
| Build (no warnings) | Rust, TypeScript, Python | Yes |
| All tests pass | Rust (cargo-nextest), TypeScript (vitest), Python (uv run pytest) | Yes |
| Linter clean | Rust (clippy), TypeScript (eslint), Python (uv run ruff check) | Yes |
| Formatter check | Rust (rustfmt), TypeScript (prettier), Python (uv run ruff format --check) | Yes |
| Type check | TypeScript (svelte-check), Python (uv run mypy) | Yes |
| Security audit | Rust (cargo audit) | Yes |
| License check | Rust (cargo deny) | Yes |
| Coverage threshold | Rust (cargo-llvm-cov, 80% core), TS (vitest, 70% state/api), Python (pytest-cov, 90% signing) | Yes |
| PR review | All | Yes (1 approval minimum) |
| SBOM generation | Release builds | Yes (release only) |

---

## 20. Container & Deployment Standards

### 20.1 Docker Image Standards

**Gateway Service Dockerfile (multi-stage):**

```dockerfile
# Stage 1: Plan (cargo-chef for layer caching)
FROM rust:1.94-bookworm AS planner
RUN cargo install cargo-chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Build dependencies (cached layer)
FROM rust:1.94-bookworm AS builder
RUN cargo install cargo-chef
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked
RUN cargo auditable build --release --locked  # Embed SBOM

# Stage 3: Runtime (Chainguard static -- zero-CVE, SBOMs, Sigstore signatures)
FROM cgr.dev/chainguard/static:latest
COPY --from=builder /app/target/release/llmsmartgate /usr/local/bin/
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/llmsmartgate"]
```

**Image standards:**

| Standard | Requirement |
|----------|-------------|
| Base image | Chainguard static `[RECOMMENDED]` (zero-CVE, SBOMs, Sigstore signatures), distroless, or Alpine for minimal attack surface |
| User | Non-root (`nonroot` or UID 65534) |
| Layer count | Minimize layers. Multi-stage build. |
| Image size | Target < 50MB for gateway |
| Scanning | `trivy` (pinned to v0.69.3, see Section 17.6) + `grype` as secondary scanner. Zero critical/high CVEs. |
| Tagging | `{version}` (e.g., `1.2.3`), `latest`, `{git-sha}`. No `latest` in production manifests. |
| Registry | Private container registry (GitHub Container Registry, AWS ECR, etc.) |
| SBOM | Embedded via `cargo auditable`. Extractable by `trivy` or `syft`. |

### 20.2 Kubernetes Resource Standards

```yaml
# Gateway Deployment (example)
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llmsmartgate-gateway
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  template:
    spec:
      containers:
        - name: gateway
          image: registry.example.com/llmsmartgate/gateway:1.2.3
          ports:
            - containerPort: 8080
          resources:
            requests:
              cpu: "500m"
              memory: "256Mi"
            limits:
              cpu: "2000m"
              memory: "512Mi"
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /readyz
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 5
          env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: llmsmartgate-secrets
                  key: database-url
            - name: VALKEY_URL
              valueFrom:
                secretKeyRef:
                  name: llmsmartgate-secrets
                  key: valkey-url
          securityContext:
            runAsNonRoot: true
            readOnlyRootFilesystem: true
            allowPrivilegeEscalation: false
            capabilities:
              drop: ["ALL"]
```

**Resource sizing guidelines:**

| Component | CPU Request | CPU Limit | Memory Request | Memory Limit |
|-----------|-----------|-----------|---------------|-------------|
| Gateway | 500m | 2000m | 256Mi | 512Mi |
| Admin Console | 100m | 500m | 128Mi | 256Mi |
| PostgreSQL | 1000m | 4000m | 2Gi | 8Gi |
| Valkey | 500m | 2000m | 512Mi | 2Gi |

### 20.3 Helm Chart Structure

```
charts/llmsmartgate/
  Chart.yaml
  values.yaml           # Default configuration
  values-production.yaml
  templates/
    gateway-deployment.yaml
    gateway-service.yaml
    gateway-hpa.yaml
    admin-deployment.yaml
    admin-service.yaml
    ingress.yaml
    configmap.yaml
    secrets.yaml        # External secret references
    pdb.yaml
    networkpolicy.yaml
    serviceaccount.yaml
```

### 20.4 Environment Configuration

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `LLMSMARTGATE_HOST` | Listen address | `0.0.0.0` | No |
| `LLMSMARTGATE_PORT` | Listen port | `8080` | No |
| `DATABASE_URL` | PostgreSQL connection URL | -- | Yes |
| `VALKEY_URL` | Valkey connection URL | -- | Yes |
| `RUST_LOG` | Log level filter | `info` | No |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP collector endpoint | -- | No (disables tracing if unset) |
| `OTEL_SERVICE_NAME` | Service name for traces | `llmsmartgate` | No |
| `LLMSMARTGATE_PG_POOL_SIZE` | PostgreSQL pool size | `20` | No |
| `LLMSMARTGATE_VALKEY_POOL_SIZE` | Valkey pool size | `10` | No |
| `LLMSMARTGATE_ADMIN_JWT_SECRET` | JWT signing secret for admin API | -- | Yes |
| `LLMSMARTGATE_TIMESTAMP_SKEW_SECS` | Allowed timestamp skew | `300` | No |
| `LLMSMARTGATE_PROVIDER_TIMEOUT_MS` | Default provider timeout | `30000` | No |

---

## 21. Git & Branching Standards

### 21.1 Branching Model

**Trunk-based development** with short-lived feature branches:

```
main (protected, always deployable)
  |
  +-- feature/PROJ-123-add-anthropic-adapter    (short-lived, <3 days)
  +-- fix/PROJ-456-nonce-replay-race-condition
  +-- chore/PROJ-789-update-sqlx-0.8.3
  |
  +-- release/1.2.0   (cut from main, for stabilization)
```

### 21.2 Branch Rules

| Rule | Setting |
|------|---------|
| Main branch protection | Required. No direct pushes. |
| Required reviews | 1 approval minimum |
| Required CI checks | All gates from Section 19.5 |
| Branch naming | `{type}/{ticket-id}-{short-description}` |
| Branch types | `feature/`, `fix/`, `chore/`, `docs/`, `refactor/`, `perf/` |
| Merge strategy | Squash merge (feature branches). Merge commit (release branches). |
| Branch lifetime | Maximum 5 business days. Prefer < 3 days. |
| Stale branches | Auto-delete after merge. |

### 21.3 Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types**: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `test`, `ci`, `build`

**Scopes**: `auth`, `policy`, `routing`, `provider`, `streaming`, `usage`, `audit`, `admin`, `sdk`, `frontend`, `infra`, `deps`

**Examples:**

```
feat(auth): implement Ed25519 signature verification

fix(streaming): cancel upstream request on client disconnect

chore(deps): update sqlx to 0.8.3

perf(routing): cache route resolution in moka with 60s TTL
```

### 21.4 Release Process

1. Cut `release/X.Y.Z` branch from `main`.
2. Only bug fixes cherry-picked into release branch.
3. Tag `vX.Y.Z` on release branch.
4. CI builds tagged release artifacts (Docker image, SBOM, Python wheel).
5. Merge release branch back to `main`.
6. Semantic versioning:
   - **Major**: Breaking API changes (rare for self-hosted gateway).
   - **Minor**: New features, new provider adapters, new admin endpoints.
   - **Patch**: Bug fixes, security patches, dependency updates.

### 21.5 Git Hooks

**Tool**: Lefthook `[RECOMMENDED]` -- Go binary, no Node.js dependency, parallel hook execution (~10x faster than Husky), YAML config works across Rust/Node/Python contexts.

```yaml
# lefthook.yml
pre-commit:
  parallel: true
  commands:
    rust-fmt:
      glob: "gateway/**/*.rs"
      run: cd gateway && cargo fmt -- --check
    rust-clippy:
      glob: "gateway/**/*.rs"
      run: cd gateway && cargo clippy --all-targets -- -D warnings
    frontend-lint:
      glob: "admin-console/**/*.{ts,svelte}"
      run: cd admin-console && pnpm lint
    frontend-format:
      glob: "admin-console/**/*.{ts,svelte}"
      run: cd admin-console && pnpm format --check
    python-lint:
      glob: "sdk-python/**/*.py"
      run: cd sdk-python && uv run ruff check && uv run ruff format --check

pre-push:
  commands:
    rust-test:
      run: cd gateway && cargo nextest run
    frontend-check:
      run: cd admin-console && pnpm check
```

### 21.6 Monorepo Orchestration

Each component is a **self-contained project** with its own build tooling:

| Component | Directory | Build tool | Managed by |
|-----------|-----------|-----------|------------|
| Gateway | `gateway/` | Cargo | Rust toolchain |
| Admin Console | `admin-console/` | pnpm + Vite | Node.js |
| Python SDK | `sdk-python/` | uv | Python |

**Orchestration**: Manual via Justfile at the repo root. No Turborepo/Nx -- the three codebases have independent build systems and are orchestrated via simple commands and CI path-based triggers.

```justfile
# Justfile (repo root)

# Run all checks (used by CI)
check-all: check-gateway check-admin check-sdk

check-gateway:
    cd gateway && cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo nextest run

check-admin:
    cd admin-console && pnpm lint && pnpm format --check && pnpm check && pnpm test

check-sdk:
    cd sdk-python && uv run ruff check && uv run ruff format --check && uv run mypy src/ && uv run pytest

# Local dev environment
dev-infra:
    docker compose up -d postgres valkey

dev-gateway:
    cd gateway && cargo run

dev-admin:
    cd admin-console && pnpm dev
```

**Cargo workspace** inside `gateway/` uses centralized dependency and lint management:

```toml
# gateway/Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"  # Automatic default in edition 2024

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# Centralized version management -- crates reference via { workspace = true }
axum = "0.8"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio", "tls-rustls", "uuid", "chrono", "json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
# ... all shared dependencies declared here

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "deny" }
pedantic = { level = "warn" }
```

### 21.7 Repository Structure

```
llmsmartgate/
  # ── Repo-level config (shared across all components) ──
  Justfile                # Monorepo task orchestration
  lefthook.yml            # Git hooks (Lefthook)
  renovate.json           # Renovate dependency update config
  docker-compose.yml      # Local dev infrastructure (PG, Valkey)
  .env.example            # Template for local environment variables
  .editorconfig           # Consistent editor settings across languages

  # ── Gateway Service (Rust) ──
  gateway/
    Cargo.toml            # Workspace root (workspace.dependencies, workspace.lints)
    Cargo.lock
    rust-toolchain.toml   # Enforce Rust edition 2024 / MSRV 1.85
    rustfmt.toml
    clippy.toml
    deny.toml             # cargo-deny configuration
    Dockerfile            # Multi-stage build (cargo-chef + Chainguard static)
    crates/
      gateway/            # Main gateway binary
        Cargo.toml
        src/
          main.rs
          config.rs
          api/
            mod.rs
            data_plane.rs
            admin.rs
            middleware.rs
            request_context.rs
          auth/
            mod.rs
            verifier.rs
            canonical.rs
            nonce.rs
            key_store.rs
            context.rs
          policy/
            mod.rs
            engine.rs
            model_access.rs
            quotas.rs
            budgets.rs
            features.rs
          routing/
            mod.rs
            router.rs
            resolver.rs
            retry.rs
            fallback.rs
            health.rs
          providers/
            mod.rs
            traits.rs
            openai.rs
            anthropic.rs
            gemini.rs
            azure_openai.rs
            vllm.rs
          streaming/
            mod.rs
            relay.rs
            events.rs
            parser.rs
            cancel.rs
          usage/
            mod.rs
            meter.rs
            pricing.rs
            aggregator.rs
          audit/
            mod.rs
            logger.rs
            event.rs
          storage/
            mod.rs
            postgres.rs
            redis.rs
            repositories/
              mod.rs
              tenants.rs
              service_accounts.rs
              keys.rs
              policies.rs
              routes.rs
              usage.rs
              audit.rs
          observability/
            mod.rs
            metrics.rs
            tracing.rs
            logging.rs
    tests/
      integration/
        auth_test.rs
        policy_test.rs
        routing_test.rs
        streaming_test.rs
    migrations/
      20260401000001_create_tenants.sql
      20260401000002_create_policies.sql
      ...

  # ── Admin Console (SvelteKit) ──
  admin-console/
    package.json
    pnpm-lock.yaml
    svelte.config.js
    vite.config.ts
    tsconfig.json
    eslint.config.js      # ESLint 10 flat config
    Dockerfile            # SvelteKit SSR or static build
    src/
      lib/
        components/
          ui/             # shadcn-svelte (DO NOT manually edit)
          app/            # Application-specific components
        state/            # Reactive classes (.svelte.ts)
        api/              # API client layer
        types/            # TypeScript type definitions
        utils/            # Pure utility functions
      routes/
        +layout.svelte
        +layout.server.ts
        dashboard/
        tenants/
        service-accounts/
        keys/
        policies/
        routes/
        usage/
        audit/
    tests/
      e2e/                # Playwright E2E tests
    static/               # Static assets

  # ── Python SDK ──
  sdk-python/
    pyproject.toml        # uv_build backend, PEP 621 metadata
    uv.lock               # Cross-platform lockfile (committed to git)
    .python-version       # Default Python version for uv
    src/
      llmsmartgate/
        __init__.py
        client.py
        async_client.py
        signer.py
        models.py
        streaming.py
    tests/
      ...

  # ── Documentation ──
  docs/
    PRD-001_prd_detail.md
    ARC-001_architecture_technology_standards.md
    HLD-001_high_level_design.md
    LLD-BE-001_backend_low_level_design.md
    LLD-FE-001_frontend_low_level_design.md
    TST-001_test_cases_checklist.md

  # ── Deployment ──
  deploy/
    helm/
      charts/llmsmartgate/
        ...

  # ── CI/CD ──
  .github/
    workflows/
      ci.yml
      release.yml
      security-audit.yml
```

---

## Appendix A: Research Sources

The technology decisions in this document are grounded in the following research (conducted April 2026):

### Rust Web Frameworks
- [Rust Web Frameworks in 2026: Axum vs Actix Web vs Rocket vs Warp vs Salvo](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2)
- [Axum vs Actix-web vs Rocket: Rust Web Framework Comparison 2026](https://reintech.io/blog/axum-vs-actix-web-vs-rocket-rust-framework-comparison-2026)
- [Axum vs. Actix Web: The 2025 Rust Web Framework War](https://medium.com/@indrajit7448/axum-vs-actix-web-the-2025-rust-web-framework-war-performance-vs-dx-17d0ccadd75e)
- [Announcing axum 0.8.0](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0)
- [Rust Web Frameworks Compared: Actix vs Axum vs Rocket](https://dev.to/leapcell/rust-web-frameworks-compared-actix-vs-axum-vs-rocket-4bad)

### Tokio Ecosystem
- [The Evolution of Async Rust: From Tokio to High-Level Applications](https://blog.jetbrains.com/rust/2026/02/17/the-evolution-of-async-rust-from-tokio-to-high-level-applications/)
- [The State of Rust Ecosystem 2025](https://blog.jetbrains.com/rust/2026/02/11/state-of-rust-2025/)
- [The State of Async Rust: Runtimes](https://corrode.dev/blog/async/)
- [TokioConf Update 2026](https://tokio.rs/blog/2026-03-03-tokioconf-update)

### Database Libraries
- [Rust ORMs in 2026: Diesel vs SQLx vs SeaORM vs Rusqlite](https://aarambhdevhub.medium.com/rust-orms-in-2026-diesel-vs-sqlx-vs-seaorm-vs-rusqlite-which-one-should-you-actually-use-706d0fe912f3)
- [Diesel vs SQLx vs SeaORM: Rust Database Library Comparison 2026](https://reintech.io/blog/diesel-vs-sqlx-vs-seaorm-rust-database-library-comparison-2026)
- [How to Build Connection Pools with bb8 and deadpool in Rust](https://oneuptime.com/blog/post/2026-01-25-connection-pools-bb8-deadpool-rust/view)

### Redis Alternatives
- [Valkey vs KeyDB vs Dragonfly: Redis Alternatives 2026](https://www.pkgpulse.com/blog/valkey-vs-keydb-vs-dragonfly-redis-alternatives-2026)
- [Redis vs DragonflyDB vs KeyDB: Best Redis Alternative in 2026?](https://singhajit.com/redis-vs-dragonflydb-vs-keydb/)
- [Redis vs Valkey vs DragonflyDB vs KeyDB Benchmarks](https://www.repoflow.io/blog/redis-vs-valkey-vs-dragonflydb-vs-keydb-benchmarks)

### PostgreSQL Performance
- [PostgreSQL Performance Tuning: Essential 2026 Expert Guide](https://www.zignuts.com/blog/postgresql-performance-tuning)
- [PostgreSQL Performance Tuning Checklist: 2026 Complete Guide](https://dev.to/_d7eb1c1703182e3ce1782/postgresql-performance-tuning-checklist-2026-complete-guide-65a)
- [PostgreSQL Performance Tuning Best Practices 2025](https://www.mydbops.com/blog/postgresql-parameter-tuning-best-practices)

### SvelteKit & Svelte 5
- [Svelte Best Practices in 2026: Scaling with Runes, Snippets, and Pure Reactivity](https://onehorizon.ai/blog/svelte-best-practices-in-2026-scaling-with-runes-snippets-and-pure-reactivity)
- [Svelte and SvelteKit Updates: Summer 2025 Recap](https://blog.openreplay.com/svelte-sveltekit-updates-summer-2025-recap/)
- [SvelteKit 2025: Modern Development Trends and Best Practices with Svelte 5](https://zxce3.net/posts/sveltekit-2025-modern-development-trends-and-best-practices/)
- [shadcn-svelte Documentation](https://www.shadcn-svelte.com/docs)

### OpenTelemetry for Rust
- [OpenTelemetry Rust Documentation](https://opentelemetry.io/docs/languages/rust/)
- [Rust Observability: Logging, Tracing, and Metrics with OpenTelemetry and Tokio](https://dasroot.net/posts/2026/01/rust-observability-opentelemetry-tokio/)
- [How to Instrument Rust Applications with OpenTelemetry](https://oneuptime.com/blog/post/2026-02-20-rust-opentelemetry-tracing/view)

### Cryptography & Post-Quantum
- [Post-Quantum Cryptography for Authentication: The Enterprise Migration Guide 2026](https://securityboulevard.com/2026/03/post-quantum-cryptography-for-authentication-the-enterprise-migration-guide-2026/)
- [State of the post-quantum Internet in 2025](https://blog.cloudflare.com/pq-2025/)
- [Cryptography in Rust: Implementing NIST's Post-Quantum Standards (2025 Edition)](https://markaicode.com/rust-post-quantum-cryptography/)

### Error Handling
- [How to Design Error Types with thiserror and anyhow in Rust](https://oneuptime.com/blog/post/2026-01-25-error-types-thiserror-anyhow-rust/view)
- [Rust Error Handling Guide 2025: New Techniques and Best Practices](https://markaicode.com/rust-error-handling-2025-guide/)

### Supply Chain Security
- [Beyond Cargo Audit: Securing Your Rust Crates in Container Images](https://anchore.com/blog/beyond-cargo-audit-securing-your-rust-crates-in-container-images/)
- [cargo-auditable: Make production Rust binaries auditable](https://github.com/rust-secure-code/cargo-auditable)
- [SBOM Generation Guide for Rust](https://sbomify.com/guides/rust/)

### HTTP Client
- [reqwest Tutorial: HTTP Client Best Practices in Rust](https://reintech.io/blog/reqwest-tutorial-http-client-best-practices-rust)
- [How to Build Resilient HTTP Clients with Retry Policies in Rust](https://oneuptime.com/blog/post/2026-01-25-resilient-http-clients-retry-policies-rust/view)

### Python SDK
- [10 Best Python HTTP Clients in 2026](https://iproyal.com/blog/best-python-http-clients/)
- [HTTPX vs Requests vs AIOHTTP - Feature and Performance Comparison](https://proxywing.com/blog/httpx-vs-requests-vs-aiohttp-feature-performance-comparison-guide)

### Tower Middleware
- [How to Build Tower Middleware for Auth and Logging in Axum](https://oneuptime.com/blog/post/2026-01-25-tower-middleware-auth-logging-axum-rust/view)
- [How to Build Production-Ready REST APIs in Rust with Axum](https://oneuptime.com/blog/post/2026-01-07-rust-axum-rest-api/view)

---

*End of Document*
