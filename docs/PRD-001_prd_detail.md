# LLMSmartGate -- Detailed Product Requirements Document

> **Doc ID:** `PRD-001`  
> **Version:** 2.0  
> **Date:** 2026-04-01  
> **Status:** Draft  
> **Classification:** Internal -- Confidential

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Business Analyst |
| **Perspective** | Business requirements, user needs, success metrics |
| **Primary Audience** | Product Owner, Stakeholders, Engineering Leads, QA |
| **Secondary Audience** | All engineering team members |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| `[MoSCoW: Must]` | Mandatory for MVP launch |
| `[MoSCoW: Should]` | Expected in production release |
| `[MoSCoW: Could]` | Nice-to-have, future consideration |
| `[Phase N]` | Delivery phase (1 = MVP, 2 = Hardening, 3 = Advanced) |
| `AC-xxx` | Acceptance Criteria ID -- traceable to test cases in `TST-001` |
| `US-xxx` | User Story ID |

### Document Relationships

```
PRD-001 (this)         -- What & Why
  ├── ARC-001          -- Architecture & Technology Standards
  ├── HLD-001          -- High-Level Design (How, big picture)
  ├── LLD-BE-001       -- Backend Low-Level Design (How, Rust detail)
  ├── LLD-FE-001       -- Frontend Low-Level Design (How, Svelte detail)
  └── TST-001          -- Test Cases & Checklists (Verify)
```

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Product Vision and Goals](#2-product-vision-and-goals)
3. [Target Users and Personas](#3-target-users-and-personas)
4. [Problem Statement](#4-problem-statement)
5. [Functional Requirements](#5-functional-requirements)
6. [Non-Functional Requirements](#6-non-functional-requirements)
7. [System Constraints and Assumptions](#7-system-constraints-and-assumptions)
8. [Dependencies and Integrations](#8-dependencies-and-integrations)
9. [Data Requirements](#9-data-requirements)
10. [Security Requirements](#10-security-requirements)
11. [Compliance and Regulatory Considerations](#11-compliance-and-regulatory-considerations)
12. [MVP Scope vs Future Phases](#12-mvp-scope-vs-future-phases)
13. [Success Metrics and KPIs](#13-success-metrics-and-kpis)
14. [Risk Register](#14-risk-register)
15. [Glossary](#15-glossary)
16. [References](#16-references)

---

## 1. Executive Summary

LLMSmartGate is a **self-hosted, high-performance LLM API Gateway** built in Rust that serves as a unified control plane and data plane for all LLM interactions within an organization. It provides an OpenAI-compatible API surface that abstracts away multi-provider complexity (OpenAI, Anthropic, Google Gemini, Azure OpenAI, vLLM/local models), enforces security policies through Ed25519 asymmetric request signing, manages cost and budget controls per tenant, and delivers full observability via OpenTelemetry-native instrumentation.

Enterprise spending on LLM APIs has grown from $3.5 billion to $8.4 billion between late 2024 and mid-2025, with 72% of organizations planning further increases (source: Pluralsight/industry reports, 2025). Without a centralized gateway, organizations face uncontrolled cost sprawl, inconsistent security postures, zero cross-service observability, and vendor lock-in. LLMSmartGate addresses these challenges by providing a single enforcement point for authentication, authorization, rate limiting, budget control, routing, and observability -- reducing operational risk while enabling teams to move faster.

The product consists of three deliverables:
- **Gateway Server** (Rust, Axum/Tokio): The core proxy handling data plane and admin control plane APIs.
- **Admin Console** (SvelteKit + shadcn-svelte): A web UI for managing tenants, service accounts, policies, routes, and viewing usage analytics.
- **Python SDK** (sync + async): A client library that handles request signing, streaming, retries, and transport fallback.

---

## 2. Product Vision and Goals

### 2.1 Vision Statement

Become the standard internal infrastructure layer for LLM access governance -- enabling any team to consume any LLM provider through a single, secure, observable, and cost-controlled API without modifying their application code beyond swapping an endpoint URL.

### 2.2 Strategic Goals

| ID | Goal | Measurable Target |
|----|------|-------------------|
| G-1 | **Unified API surface** | 100% of LLM requests routed through the gateway within 90 days of deployment |
| G-2 | **Security by default** | Zero plaintext API keys in application code; all requests cryptographically signed |
| G-3 | **Cost visibility and control** | Per-tenant, per-service-account cost attribution with budget enforcement within $0.01 accuracy |
| G-4 | **Provider resilience** | Automatic failover achieving 99.9% effective availability even when individual providers experience outages |
| G-5 | **Minimal overhead** | Gateway-added latency under 10ms at p50, under 50ms at p99 (excluding provider round-trip) |
| G-6 | **Full observability** | Every request traceable end-to-end via OpenTelemetry with correlation IDs, structured logs, and usage metrics |
| G-7 | **Developer velocity** | SDK integration achievable in under 30 minutes with fewer than 10 lines of code change |

### 2.3 Non-Goals (Explicitly Out of Scope)

- Building a chat UI product for end users.
- Hosting or training models (the gateway proxies to providers/existing infrastructure).
- Replacing existing CI/CD or deployment pipelines.
- Providing a general-purpose API gateway for non-LLM traffic.
- Building a vector database or RAG pipeline (the gateway handles the LLM call layer only).

---

## 3. Target Users and Personas

### 3.1 Persona: Platform Engineer (Primary)

**Role:** Owns internal developer platform and infrastructure.

| Attribute | Detail |
|-----------|--------|
| Goals | Centralize LLM access, enforce security policies, manage provider credentials, ensure uptime |
| Pain points | API key sprawl across services, no visibility into LLM spend, manual provider failover, inconsistent auth |
| Usage pattern | Configures gateway via Admin Console and Admin API; sets up tenants, routes, policies |
| Success criteria | Single pane of glass for all LLM infrastructure; zero unmanaged API keys |

### 3.2 Persona: Backend Developer (Primary)

**Role:** Builds applications and services that consume LLMs.

| Attribute | Detail |
|-----------|--------|
| Goals | Call LLMs with minimal setup; not worry about auth, retries, streaming, or provider differences |
| Pain points | Each provider has a different SDK; handling retries and streaming is error-prone; unclear cost attribution |
| Usage pattern | Integrates Python SDK; calls OpenAI-compatible endpoints; uses model aliases |
| Success criteria | Drop-in replacement for direct OpenAI SDK calls; transparent retries and fallback |

### 3.3 Persona: Security / Compliance Engineer (Secondary)

**Role:** Ensures API security posture, audits key usage, monitors for anomalies.

| Attribute | Detail |
|-----------|--------|
| Goals | Audit trail for all LLM interactions; key rotation enforcement; policy compliance verification |
| Pain points | No centralized audit log; keys shared informally; no budget guardrails |
| Usage pattern | Reviews audit logs in Admin Console; configures policies; monitors alerts |
| Success criteria | Immutable audit trail; automated key expiry alerts; budget breach notifications |

### 3.4 Persona: Engineering Manager / FinOps (Secondary)

**Role:** Manages team budgets and oversees LLM cost allocation.

| Attribute | Detail |
|-----------|--------|
| Goals | Understand LLM spend per team/project; set budget limits; forecast costs |
| Pain points | No per-team cost breakdown; surprise bills; no ability to set spending limits |
| Usage pattern | Views dashboards in Admin Console; receives budget alerts |
| Success criteria | Cost attribution to the service-account level; automated budget enforcement |

---

## 4. Problem Statement

### 4.1 Current State

Organizations adopting LLMs face a fragmented landscape:

1. **Security gaps:** Individual teams embed provider API keys directly in application code or environment variables. There is no centralized key management, rotation policy, or cryptographic request authentication. This violates OWASP API2:2023 (Broken Authentication) and introduces secrets sprawl.

2. **Cost blindness:** With no metering layer, teams cannot attribute LLM spend to specific services or workloads. Budget overruns are discovered only at invoice time. Industry data shows organizations can reduce LLM spend by 47-80% with proper metering and optimization (source: Pluralsight, 2025).

3. **Provider lock-in and fragility:** Applications are tightly coupled to a single provider's SDK and API format. When that provider experiences downtime (which major providers experience multiple times per year), the entire dependent application fails. There is no automated failover.

4. **Observability gaps:** LLM calls are invisible to existing application monitoring. Token counts, latency, error rates, and cost per request are not captured in centralized telemetry. This makes debugging, capacity planning, and SLO tracking impossible.

5. **Policy vacuum:** There is no way to enforce which models a team can use, limit token consumption, restrict streaming or tool-use capabilities, or gate access based on environment (dev/staging/prod).

### 4.2 Desired Future State

A single, self-hosted gateway through which all LLM traffic flows, providing:
- Cryptographic authentication for every request (no shared secrets).
- Fine-grained policy enforcement (model access, token limits, budget caps, feature flags).
- Intelligent multi-provider routing with automatic failover.
- Real-time cost attribution and budget enforcement.
- Full OpenTelemetry-native observability with correlation from client SDK to provider response.
- An admin console for operational management.

---

## 5. Functional Requirements

### 5.1 API Layer (Data Plane)

#### FR-1.1: OpenAI-Compatible Chat Completions Endpoint

**User Story:** As a backend developer, I want to send chat completion requests to the gateway using the same schema as the OpenAI API, so that I can migrate existing code with minimal changes.

**Endpoint:** `POST /v1/chat/completions`

**Acceptance Criteria:**
- AC-1: The endpoint accepts a JSON body conforming to the OpenAI ChatCompletions request schema (model, messages, temperature, top_p, max_tokens, stream, tools, tool_choice, response_format).
- AC-2: When `stream: false`, the response is a single JSON object matching the OpenAI ChatCompletions response schema (id, object, created, model, choices, usage).
- AC-3: When `stream: true`, the response is an SSE stream of `data: {...}` events conforming to the OpenAI streaming delta format, terminated by `data: [DONE]`.
- AC-4: Invalid request bodies return HTTP 422 with a structured error response including `error.code`, `error.message`, and `request_id`.
- AC-5: The `model` field is interpreted as a model alias and resolved by the routing engine.
- AC-6: The response includes gateway-specific headers: `X-Request-Id`, `X-Trace-Id`, `X-Gateway-Latency-Ms`.

#### FR-1.2: Responses API Endpoint

**User Story:** As a backend developer, I want to use the OpenAI Responses API format through the gateway, so that I can leverage newer API patterns without changing providers.

**Endpoint:** `POST /v1/responses`

**Acceptance Criteria:**
- AC-1: The endpoint accepts a JSON body with fields: model, input (string or array), stream, max_output_tokens, tools, instructions, previous_response_id.
- AC-2: Non-streaming responses return a JSON object with id, object ("response"), model, output, output_text, usage (input_tokens, output_tokens, total_tokens).
- AC-3: Streaming responses emit SSE events: `response.created`, `response.output_text.delta`, `response.output_text.done`, `response.completed`.
- AC-4: Tool-use responses include tool call outputs in the output array.
- AC-5: The provider adapter translates this format to the appropriate provider-native format (e.g., Anthropic Messages API, Gemini GenerateContent).

#### FR-1.3: Embeddings Endpoint

**User Story:** As a backend developer, I want to generate embeddings through the gateway so that all my LLM-related API calls are routed and metered centrally.

**Endpoint:** `POST /v1/embeddings`

**Acceptance Criteria:**
- AC-1: The endpoint accepts model and input (string or array of strings/token arrays).
- AC-2: The response conforms to the OpenAI Embeddings response schema (object, data, model, usage).
- AC-3: Embeddings are metered for token usage and cost attribution.
- AC-4: The endpoint supports routing to different embedding providers via model alias.

#### FR-1.4: Models Listing Endpoint

**User Story:** As a backend developer, I want to list available model aliases so that I can discover which models are configured and available for my service account.

**Endpoint:** `GET /v1/models`

**Acceptance Criteria:**
- AC-1: Returns a list of model aliases the authenticated service account is authorized to use (intersection of route configuration and policy allowed_models).
- AC-2: Response format matches OpenAI models list schema (object: "list", data: array of model objects).
- AC-3: Each model object includes id (the alias), object ("model"), created, owned_by (provider name).

#### FR-1.5: Request Validation

**User Story:** As a platform engineer, I want the gateway to validate all incoming requests before forwarding them to providers, so that malformed requests are rejected early and do not consume provider quota.

**Acceptance Criteria:**
- AC-1: All required fields are validated against the OpenAPI schema; missing fields return 422.
- AC-2: Content-Type must be `application/json`; other types return 415.
- AC-3: Request body size is limited to a configurable maximum (default: 10 MB); exceeding returns 413.
- AC-4: JSON parsing errors return 400 with a descriptive error message.
- AC-5: Unknown fields are passed through (forward-compatible) but logged.

---

### 5.2 Authentication and Security

#### FR-2.1: Ed25519 Asymmetric Request Signing

**User Story:** As a security engineer, I want every data plane request to be cryptographically signed using Ed25519, so that we can verify the identity and integrity of each request without transmitting secrets.

**Acceptance Criteria:**
- AC-1: Every data plane request must include headers: `X-Service-Account-Id`, `X-Key-Id`, `X-Timestamp`, `X-Nonce`, `X-Body-SHA256`, `X-Signature`.
- AC-2: Missing any required header returns HTTP 401 with error code `missing_auth_header`.
- AC-3: The gateway reconstructs the canonical signing string as: `HTTP_METHOD\nREQUEST_PATH\nTIMESTAMP\nNONCE\nBODY_SHA256`.
- AC-4: The signature is verified against the public key registered for the given `key_id`.
- AC-5: If the signature does not match, return HTTP 401 with error code `invalid_signature`.
- AC-6: The signing algorithm is Ed25519 (RFC 8032); no other algorithms are accepted in v1.

#### FR-2.2: Timestamp Validation

**User Story:** As a security engineer, I want request timestamps to be validated within a tight window, so that captured requests cannot be replayed after the window expires.

**Acceptance Criteria:**
- AC-1: The `X-Timestamp` header must be within +/- 300 seconds (5 minutes) of the gateway's server time.
- AC-2: Requests outside the window are rejected with HTTP 401 and error code `timestamp_expired`.
- AC-3: The window size is configurable via gateway configuration (minimum: 60 seconds, maximum: 600 seconds).
- AC-4: The gateway clock must be synchronized via NTP with drift under 1 second.

#### FR-2.3: Nonce Replay Protection

**User Story:** As a security engineer, I want each request nonce to be unique, so that an intercepted signed request cannot be replayed within the timestamp window.

**Acceptance Criteria:**
- AC-1: The gateway stores each `(service_account_id, nonce)` pair in Redis with a TTL equal to the timestamp window (default: 300 seconds).
- AC-2: If a duplicate nonce is detected for the same service account within the TTL, the request is rejected with HTTP 401 and error code `nonce_replay`.
- AC-3: Nonce values must be at least 16 characters and at most 64 characters.
- AC-4: Redis nonce storage must handle at least 100,000 nonce checks per second with sub-millisecond latency.

#### FR-2.4: Body Integrity Verification

**User Story:** As a security engineer, I want the request body hash to be verified, so that any tampering with the request payload after signing is detected.

**Acceptance Criteria:**
- AC-1: The `X-Body-SHA256` header must match the SHA-256 hash of the raw request body.
- AC-2: Hash mismatch returns HTTP 401 with error code `body_hash_mismatch`.
- AC-3: For requests with no body (e.g., GET), the hash of an empty string is used.

#### FR-2.5: TLS Enforcement

**User Story:** As a security engineer, I want all gateway traffic to be encrypted in transit, so that request signing headers and payloads cannot be intercepted.

**Acceptance Criteria:**
- AC-1: The gateway only accepts HTTPS connections (TLS 1.2 minimum, TLS 1.3 preferred).
- AC-2: HTTP requests receive a 301 redirect to HTTPS, or are rejected with 403 (configurable).
- AC-3: TLS 0-RTT (early data) is disabled to prevent replay attacks at the transport layer.
- AC-4: Cipher suites are restricted to AEAD ciphers (AES-256-GCM, ChaCha20-Poly1305).

---

### 5.3 Service Account Management

#### FR-3.1: Create Service Account

**User Story:** As a platform engineer, I want to create service accounts representing workload identities, so that each application or service has its own identity for LLM access.

**Acceptance Criteria:**
- AC-1: A service account belongs to exactly one tenant.
- AC-2: Required fields: name, slug (unique within tenant), environment (dev/staging/prod).
- AC-3: Optional fields: description, default_policy_id.
- AC-4: The slug is validated: lowercase alphanumeric plus hyphens, 3-63 characters.
- AC-5: Creation emits an audit event with action `service_account.created`.
- AC-6: The response includes the service account ID (UUID v4).

#### FR-3.2: Register Public Key

**User Story:** As a platform engineer, I want to register Ed25519 public keys for a service account, so that the service can sign requests.

**Acceptance Criteria:**
- AC-1: A service account can have multiple active keys (for rotation).
- AC-2: Required fields: public_key_pem (Ed25519 public key in PEM format), key_id (client-specified, unique globally).
- AC-3: Optional fields: expires_at (key expiration timestamp).
- AC-4: The gateway validates the PEM format and key length (Ed25519 = 32 bytes public key).
- AC-5: The key fingerprint is computed as SHA-256 of the raw public key bytes and stored.
- AC-6: Creation emits an audit event with action `key.registered`.
- AC-7: Maximum 10 active keys per service account (configurable).

#### FR-3.3: Rotate and Revoke Keys

**User Story:** As a platform engineer, I want to rotate keys with zero downtime and revoke compromised keys immediately, so that security incidents can be mitigated without service disruption.

**Acceptance Criteria:**
- AC-1: Key rotation workflow: register new key -> deploy to client -> revoke old key.
- AC-2: A key can be set to status `rotating` (accepted but generates a warning log) before being revoked.
- AC-3: Revoking a key sets status to `revoked` and `revoked_at` timestamp; the key is immediately rejected for new requests.
- AC-4: Revocation emits an audit event with action `key.revoked`.
- AC-5: Expired keys (past `expires_at`) are automatically rejected; a background job marks them as `expired` status.
- AC-6: The `last_used_at` field is updated on every successful authentication (debounced to max once per 60 seconds to reduce write load).

#### FR-3.4: Service Account Suspension

**User Story:** As a platform engineer, I want to suspend a service account to immediately block all its requests, so that I can respond to security incidents or policy violations.

**Acceptance Criteria:**
- AC-1: Setting status to `suspended` causes all requests from that service account to be rejected with HTTP 403 and error code `service_account_suspended`.
- AC-2: Suspension takes effect within 5 seconds (cache invalidation).
- AC-3: Suspension emits an audit event with action `service_account.suspended`.
- AC-4: Reactivation is possible by setting status back to `active`.

---

### 5.4 Policy Engine

#### FR-4.1: Model Access Control

**User Story:** As a platform engineer, I want to control which models each service account can access, so that teams only use approved models.

**Acceptance Criteria:**
- AC-1: Policies define `allowed_models` (whitelist) and `denied_models` (blacklist) as arrays of model alias patterns.
- AC-2: Patterns support exact match and glob wildcards (e.g., `gpt-4*`, `claude-*`).
- AC-3: Deny takes precedence over allow.
- AC-4: If `allowed_models` is empty, all models not in `denied_models` are allowed.
- AC-5: A request for a denied model returns HTTP 403 with error code `model_not_allowed` and the policy name in the response.

#### FR-4.2: Token Limits

**User Story:** As a platform engineer, I want to enforce maximum token limits per request, so that individual requests cannot consume excessive resources.

**Acceptance Criteria:**
- AC-1: Policies define `max_input_tokens` and `max_output_tokens`.
- AC-2: If the request specifies `max_tokens` or `max_output_tokens` exceeding the policy limit, the gateway clamps it to the policy limit (not rejection) and adds a response header `X-Policy-Max-Tokens-Applied: true`.
- AC-3: Input token count is estimated before sending to the provider using a fast tokenizer (tiktoken-compatible for OpenAI models, approximate character-based for others).
- AC-4: If estimated input tokens exceed `max_input_tokens`, the request is rejected with HTTP 403 and error code `input_tokens_exceeded`.

#### FR-4.3: Rate Limiting

**User Story:** As a platform engineer, I want to enforce rate limits per service account, so that no single consumer can overwhelm the gateway or exhaust provider quotas.

**Acceptance Criteria:**
- AC-1: Rate limits are defined as requests per minute (RPM) in the policy.
- AC-2: Rate limiting uses a sliding window algorithm implemented in Redis.
- AC-3: When rate limit is exceeded, the gateway returns HTTP 429 with headers: `Retry-After` (seconds), `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`.
- AC-4: Rate limits can be set at three levels (evaluated in order): service account, tenant, global.
- AC-5: The most restrictive applicable limit is enforced.

#### FR-4.4: Budget Enforcement

**User Story:** As an engineering manager, I want to set daily and monthly budget limits per service account or tenant, so that we never exceed our allocated LLM spend.

**Acceptance Criteria:**
- AC-1: Budgets are defined with `period_type` (daily, monthly, custom), `amount_limit` (in USD), and scope (tenant-level or service-account-level).
- AC-2: Budget consumption is tracked in near-real-time (within 5 seconds of request completion).
- AC-3: When a budget is 80% consumed, a warning event is emitted (for alerting).
- AC-4: When a budget is 100% consumed, new requests are rejected with HTTP 403 and error code `budget_exhausted`, including the budget name and current spend in the response.
- AC-5: Budget tracking is based on estimated cost computed from token counts and the configured pricing table.
- AC-6: Budget resets automatically at the start of each period (daily at 00:00 UTC, monthly on the 1st).

#### FR-4.5: Feature Flags

**User Story:** As a platform engineer, I want to control whether a service account can use streaming, tools, or file inputs, so that I can enforce compliance requirements.

**Acceptance Criteria:**
- AC-1: Policies define boolean flags: `allow_streaming`, `allow_tools`, `allow_files`.
- AC-2: If `allow_streaming` is false and the request has `stream: true`, the gateway rejects with HTTP 403 and error code `streaming_not_allowed`.
- AC-3: If `allow_tools` is false and the request includes `tools` or `tool_choice`, the gateway rejects with HTTP 403 and error code `tools_not_allowed`.
- AC-4: If `allow_files` is false and the request includes file/image content parts, the gateway rejects with HTTP 403 and error code `files_not_allowed`.

#### FR-4.6: Policy Binding

**User Story:** As a platform engineer, I want to attach multiple policies to a service account with a merge strategy, so that I can compose base policies with team-specific overrides.

**Acceptance Criteria:**
- AC-1: A service account can have multiple policies bound via `service_account_policy_bindings`.
- AC-2: A service account also has a `default_policy_id` fallback.
- AC-3: When multiple policies apply, the most restrictive value for each field is used (intersection for allowed_models, minimum for token limits and budgets, AND for feature flags).
- AC-4: Policy evaluation order: explicit bindings (by creation order) -> default policy.
- AC-5: Policy evaluation result is cached in-memory for 30 seconds (configurable) to avoid repeated DB lookups.

---

### 5.5 Routing Engine

#### FR-5.1: Model Alias Resolution

**User Story:** As a platform engineer, I want to define model aliases that map to specific provider models, so that application code uses stable aliases and I can change the backing provider without code changes.

**Acceptance Criteria:**
- AC-1: A route maps a `model_alias` (e.g., "smart-large") to a specific `provider` + `provider_model_name` (e.g., "openai" + "gpt-4o").
- AC-2: Multiple routes can exist for the same alias, with different priorities.
- AC-3: Routes can be tenant-scoped (apply only to a specific tenant) or global (apply to all tenants).
- AC-4: Tenant-scoped routes take precedence over global routes.
- AC-5: Only enabled routes are considered during resolution.

#### FR-5.2: Priority-Based Routing (v1)

**User Story:** As a platform engineer, I want requests to be routed to the highest-priority provider, so that I can control cost and quality by choosing which provider serves each model alias.

**Acceptance Criteria:**
- AC-1: Routes are sorted by `priority` (lower number = higher priority, default: 100).
- AC-2: The gateway attempts the highest-priority enabled route first.
- AC-3: If the primary route fails (5xx, timeout), the gateway attempts the next route in priority order (see FR-5.3).
- AC-4: Route configuration is cached in memory for 60 seconds (configurable), with a cache invalidation trigger on admin updates.

#### FR-5.3: Retry and Fallback

**User Story:** As a platform engineer, I want the gateway to automatically retry failed requests and fall back to alternative providers, so that transient provider failures do not impact application reliability.

**Acceptance Criteria:**
- AC-1: Each route has a configurable `max_retries` (default: 1) and `retry_backoff_ms` (default: 250ms).
- AC-2: Retries are attempted on the same provider for: HTTP 429 (after Retry-After delay), HTTP 500, HTTP 502, HTTP 503, connection timeout.
- AC-3: Retries are NOT attempted for: HTTP 400, HTTP 401, HTTP 403, HTTP 404 (client errors are terminal).
- AC-4: After exhausting retries on the primary route, the gateway attempts the next route in the fallback chain (same model alias, next priority).
- AC-5: The total request timeout (across all attempts) is configurable per route (default: 60 seconds).
- AC-6: Each retry/fallback attempt is recorded in the usage event with `retry_count` and `provider_attempts` metadata.
- AC-7: Retry uses exponential backoff with jitter: `min(retry_backoff_ms * 2^attempt + random(0, retry_backoff_ms), max_backoff_ms)`.

#### FR-5.4: Timeout Handling

**User Story:** As a backend developer, I want the gateway to enforce timeouts, so that my application is not blocked indefinitely by a slow provider.

**Acceptance Criteria:**
- AC-1: Each route has a configurable `timeout_ms` (default: 30,000ms) for the provider call.
- AC-2: For streaming requests, the timeout applies to time-to-first-byte (TTFB) from the provider; once streaming begins, an inactivity timeout of 30 seconds (configurable) applies.
- AC-3: On timeout, the gateway cancels the upstream connection and returns HTTP 504 with error code `provider_timeout`.
- AC-4: Timeout events are logged with full context (provider, model, elapsed time).

---

### 5.6 Provider Adapters

#### FR-6.1: OpenAI-Compatible Adapter

**User Story:** As a platform engineer, I want the gateway to support any OpenAI-compatible API (including vLLM, Ollama, and other local inference servers), so that I can route to both cloud and self-hosted models.

**Acceptance Criteria:**
- AC-1: The adapter supports chat completions, responses, and embeddings endpoints.
- AC-2: Authentication is via Bearer token (API key) in the Authorization header.
- AC-3: The adapter supports both streaming and non-streaming modes.
- AC-4: The adapter extracts token usage from the response (prompt_tokens, completion_tokens, total_tokens).
- AC-5: For vLLM/local models, the base URL is configurable per route.

#### FR-6.2: Anthropic Adapter

**User Story:** As a platform engineer, I want the gateway to route requests to Anthropic Claude models with automatic format translation.

**Acceptance Criteria:**
- AC-1: The adapter translates OpenAI chat format (messages array) to Anthropic Messages API format (system as top-level parameter, messages without system role).
- AC-2: The adapter handles Anthropic-specific headers: `x-api-key`, `anthropic-version`.
- AC-3: Streaming is translated from Anthropic SSE events (`content_block_delta`, `message_stop`) to OpenAI-compatible delta format.
- AC-4: Token usage is extracted from the Anthropic response (input_tokens, output_tokens).
- AC-5: Tool use is translated between OpenAI tool format and Anthropic tool_use/tool_result format.
- AC-6: The adapter supports extended thinking (Anthropic's thinking blocks) by passing through when the downstream format supports it.

#### FR-6.3: Google Gemini Adapter

**User Story:** As a platform engineer, I want the gateway to route requests to Google Gemini models.

**Acceptance Criteria:**
- AC-1: The adapter translates OpenAI chat format to Gemini GenerateContent format (contents array with parts).
- AC-2: Authentication uses the Google API key or OAuth2 service account credentials.
- AC-3: Streaming uses Gemini's SSE format and translates to OpenAI delta format.
- AC-4: Token usage is extracted from `usageMetadata` in the Gemini response.
- AC-5: System instructions are placed in the `systemInstruction` field.

#### FR-6.4: Azure OpenAI Adapter

**User Story:** As a platform engineer, I want to route to Azure OpenAI deployments, so that we can use our Azure enterprise agreement.

**Acceptance Criteria:**
- AC-1: The adapter constructs Azure-specific URLs: `https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version={version}`.
- AC-2: Authentication uses the `api-key` header (Azure format).
- AC-3: The deployment name and API version are configurable per route.
- AC-4: Response format is identical to OpenAI (no translation needed).

#### FR-6.5: Provider Error Normalization

**User Story:** As a backend developer, I want consistent error responses regardless of which provider is serving my request, so that my error handling code is simple and portable.

**Acceptance Criteria:**
- AC-1: All provider errors are mapped to a normalized error schema: `{ error: { code, message, provider_error_code, provider, request_id } }`.
- AC-2: Standard normalized error codes: `provider_timeout`, `provider_rate_limited`, `provider_auth_error`, `provider_unavailable`, `provider_bad_request`, `provider_content_filtered`, `provider_context_length_exceeded`, `provider_unknown_error`.
- AC-3: The original provider error message is included in `provider_error_code` for debugging but not exposed to the end user by default (configurable).
- AC-4: HTTP status codes are mapped consistently: provider 429 -> gateway 429, provider 500/502/503 -> gateway 502, provider 400 -> gateway 400, provider 401 -> gateway 502 (gateway misconfiguration, not client's fault).

---

### 5.7 Streaming Engine

#### FR-7.1: SSE Stream Relay

**User Story:** As a backend developer, I want to receive real-time streaming responses from the gateway, so that my application can display tokens as they are generated.

**Acceptance Criteria:**
- AC-1: When `stream: true`, the gateway opens a connection to the provider and relays chunks to the client in real-time.
- AC-2: The content-type of the streaming response is `text/event-stream`.
- AC-3: Each SSE event is prefixed with `data: ` and events are separated by `\n\n`.
- AC-4: The stream is terminated with `data: [DONE]\n\n` for chat completions format.
- AC-5: Gateway-added latency per chunk is under 1ms (passthrough, no buffering of individual chunks).

#### FR-7.2: Stream Event Normalization

**User Story:** As a backend developer, I want streaming events from all providers to follow the same format, so that my streaming parser works regardless of the backing provider.

**Acceptance Criteria:**
- AC-1: For chat completions: events follow OpenAI delta format with `choices[0].delta.content`.
- AC-2: For responses API: events follow the specified event types: `response.created`, `response.output_text.delta`, `response.output_text.done`, `response.completed`, `response.error`.
- AC-3: Provider-specific event formats (Anthropic content_block_delta, Gemini streaming chunks) are translated to the gateway's normalized format in-flight.

#### FR-7.3: Client Disconnect Handling

**User Story:** As a platform engineer, I want the gateway to cancel upstream provider requests when the client disconnects, so that we do not waste provider tokens and money on abandoned requests.

**Acceptance Criteria:**
- AC-1: The gateway monitors the client TCP connection for disconnects.
- AC-2: On client disconnect, the gateway cancels (drops) the upstream provider connection within 100ms.
- AC-3: Partial usage (tokens generated before cancellation) is still recorded in the usage event with `final_status: "client_disconnected"`.
- AC-4: The usage event includes `completion_tokens` reflecting only the tokens that were actually generated before cancellation.

#### FR-7.4: Stream Finalization

**User Story:** As a platform engineer, I want accurate usage metering even for streamed responses, so that token counts and costs are correctly attributed.

**Acceptance Criteria:**
- AC-1: For providers that include usage in the final streaming event (OpenAI: `usage` field in last chunk), the gateway extracts and records those exact counts.
- AC-2: For providers that do not include usage in the stream (some Anthropic streaming modes), the gateway accumulates an estimate during streaming and reconciles with the final `message_stop` event.
- AC-3: Usage events for streaming requests are written after the stream completes (or is cancelled).

---

### 5.8 Usage and Cost Metering

#### FR-8.1: Per-Request Usage Recording

**User Story:** As a platform engineer, I want every LLM request to be recorded with full metadata, so that I have a complete audit trail and can compute costs accurately.

**Acceptance Criteria:**
- AC-1: Every completed request (success or failure) generates a `usage_event` record.
- AC-2: The usage event includes: request_id, tenant_id, service_account_id, provider, model_alias, provider_model_name, prompt_tokens, completion_tokens, total_tokens, estimated_cost, currency (USD), latency_ms (gateway-measured end-to-end), retry_count, final_status, is_streaming, created_at.
- AC-3: Usage events are persisted to PostgreSQL asynchronously (non-blocking to the response path) with a maximum delay of 5 seconds.
- AC-4: If PostgreSQL write fails, events are buffered in memory (up to 10,000 events) and retried with exponential backoff.
- AC-5: A Prometheus/OpenTelemetry metric `usage_events_buffer_size` exposes the buffer depth for alerting.

#### FR-8.2: Cost Estimation

**User Story:** As an engineering manager, I want accurate cost estimates for each request, so that I can track spend against budgets.

**Acceptance Criteria:**
- AC-1: The gateway maintains a pricing table mapping (provider, model_name) to (input_price_per_1k_tokens, output_price_per_1k_tokens).
- AC-2: The pricing table is stored in PostgreSQL and cached in memory with a 5-minute TTL.
- AC-3: Estimated cost = (prompt_tokens / 1000 * input_price) + (completion_tokens / 1000 * output_price).
- AC-4: The pricing table is updatable via the Admin API without gateway restart.
- AC-5: If a model's price is not in the table, cost is recorded as 0 and a warning metric is emitted.

#### FR-8.3: Usage Aggregation Queries

**User Story:** As an engineering manager, I want to query aggregated usage by tenant, service account, model, provider, and time range, so that I can generate cost reports.

**Acceptance Criteria:**
- AC-1: The Admin API exposes `GET /admin/v1/usage/summary` with query parameters: tenant_id, service_account_id, model_alias, provider, start_date, end_date, group_by (tenant, service_account, model, provider, day, hour).
- AC-2: The response includes aggregated totals: total_requests, total_prompt_tokens, total_completion_tokens, total_cost, average_latency_ms, error_rate.
- AC-3: Queries for the last 90 days return within 2 seconds for tenants with up to 10 million usage events.

---

### 5.9 Rate Limiting

#### FR-9.1: Multi-Level Rate Limiting

**User Story:** As a platform engineer, I want rate limits enforced at multiple levels (global, tenant, service account), so that I can prevent any single consumer from degrading service for others.

**Acceptance Criteria:**
- AC-1: Rate limits are evaluated in order: global -> tenant -> service account. All applicable limits are checked; the first exceeded limit triggers rejection.
- AC-2: Global rate limits are configured in the gateway configuration file (not in the database).
- AC-3: Tenant and service account rate limits are configured in policies.
- AC-4: Rate limit state is stored in Redis using a sliding window counter (MULTI/EXEC for atomicity).
- AC-5: Redis rate limit operations complete within 1ms at p99.

#### FR-9.2: Rate Limit Response Headers

**User Story:** As a backend developer, I want rate limit information in response headers, so that my application can implement backoff logic proactively.

**Acceptance Criteria:**
- AC-1: Every response includes: `X-RateLimit-Limit` (requests per minute allowed), `X-RateLimit-Remaining` (requests remaining in current window), `X-RateLimit-Reset` (Unix timestamp when the window resets).
- AC-2: When rate limited (HTTP 429), the response also includes `Retry-After` (seconds to wait).
- AC-3: Headers reflect the most restrictive applicable limit.

#### FR-9.3: Concurrency Limiting

**User Story:** As a platform engineer, I want to limit concurrent in-flight requests per service account, so that a single consumer with long-running streaming requests cannot exhaust connection pools.

**Acceptance Criteria:**
- AC-1: Policies define `concurrency_limit` (max simultaneous in-flight requests).
- AC-2: Concurrency is tracked using Redis atomic counters (INCR on request start, DECR on request end).
- AC-3: When concurrency limit is exceeded, the gateway returns HTTP 429 with error code `concurrency_limit_exceeded`.
- AC-4: A safety mechanism decrements the counter even if the request handler panics (using a drop guard or finally block).

---

### 5.10 Audit Logging

#### FR-10.1: Administrative Action Auditing

**User Story:** As a security engineer, I want an immutable audit log of all administrative actions, so that I can investigate security incidents and demonstrate compliance.

**Acceptance Criteria:**
- AC-1: The following actions are audited: service_account.created, service_account.updated, service_account.suspended, service_account.deleted, key.registered, key.revoked, key.expired, policy.created, policy.updated, policy.deleted, route.created, route.updated, route.deleted, tenant.created, tenant.updated, tenant.suspended, budget.created, budget.updated, provider_config.updated, pricing.updated.
- AC-2: Each audit event includes: id, tenant_id, actor_type (admin_user, system, api), actor_id, action, target_type, target_id, metadata_json (containing before/after state for updates), created_at (immutable).
- AC-3: Audit events are append-only; no UPDATE or DELETE operations are permitted on the audit_events table.
- AC-4: The Admin API exposes `GET /admin/v1/audit-events` with filtering by tenant_id, action, target_type, actor_id, date range, with pagination (cursor-based).
- AC-5: Audit events older than the retention period (configurable, default: 365 days) can be exported to object storage (S3/GCS) before deletion.

#### FR-10.2: Security Event Auditing

**User Story:** As a security engineer, I want security-relevant data plane events to be logged, so that I can detect and investigate authentication anomalies.

**Acceptance Criteria:**
- AC-1: The following security events are logged (to structured logs, not the audit_events table for performance): auth_failure (signature mismatch, expired timestamp, nonce replay, unknown key, suspended account), policy_denial, rate_limit_exceeded, budget_exhausted.
- AC-2: Security events include: request_id, trace_id, service_account_id, key_id, client_ip, failure_reason, timestamp.
- AC-3: Security events are emitted as structured JSON logs at WARN level.
- AC-4: An OpenTelemetry metric `security_events_total{type, reason}` is incremented for each security event.

---

### 5.11 Admin Control Plane API

#### FR-11.1: Tenant Management

**User Story:** As a platform engineer, I want to create and manage tenants via the Admin API, so that I can onboard new teams or organizations.

**Endpoint Prefix:** `/admin/v1/tenants`

**Acceptance Criteria:**
- AC-1: CRUD operations for tenants: POST (create), GET (list/get), PATCH (update), DELETE (soft delete via status change).
- AC-2: Tenant creation requires: name, slug (unique). Optional: metadata.
- AC-3: Tenant deletion sets status to `deleted`; associated service accounts are suspended but data is retained per retention policy.
- AC-4: List supports pagination (cursor-based, default page size 50, max 200) and filtering by status.

#### FR-11.2: Service Account Management

**Endpoint Prefix:** `/admin/v1/service-accounts`

**Acceptance Criteria:**
- AC-1: CRUD operations scoped by tenant_id.
- AC-2: Includes sub-resources: keys (`POST /admin/v1/service-accounts/{id}/keys`, `DELETE .../keys/{key_id}`).
- AC-3: Includes sub-resources: policy bindings (`POST /admin/v1/service-accounts/{id}/policy-bindings`, `DELETE .../policy-bindings/{binding_id}`).
- AC-4: List supports filtering by tenant_id, status, environment.

#### FR-11.3: Policy Management

**Endpoint Prefix:** `/admin/v1/policies`

**Acceptance Criteria:**
- AC-1: CRUD operations scoped by tenant_id.
- AC-2: Policy updates trigger cache invalidation across all gateway instances within 5 seconds.
- AC-3: Deleting a policy that is currently bound to service accounts returns HTTP 409 (conflict) unless `force=true` is specified.

#### FR-11.4: Route Management

**Endpoint Prefix:** `/admin/v1/routes`

**Acceptance Criteria:**
- AC-1: CRUD operations for provider routes.
- AC-2: Route changes take effect within 60 seconds (cache TTL) or immediately via explicit cache invalidation endpoint.
- AC-3: Bulk import/export of routes via JSON (for infrastructure-as-code workflows).

#### FR-11.5: Admin Authentication

**User Story:** As a platform engineer, I want admin API authentication to be separate from data plane authentication, so that admin operations have appropriate access controls.

**Acceptance Criteria:**
- AC-1: Admin API supports two authentication methods: Bearer token (API key) and session-based (for the Admin Console web UI).
- AC-2: Admin API keys are stored as bcrypt hashes in PostgreSQL.
- AC-3: Session tokens are stored in Redis with a configurable TTL (default: 8 hours).
- AC-4: Admin actions are subject to RBAC with at least two roles: `admin` (full access) and `viewer` (read-only).
- AC-5: All admin authentication failures are logged as security events.

---

### 5.12 Admin Console (Frontend)

#### FR-12.1: Dashboard

**User Story:** As a platform engineer, I want a dashboard showing key metrics at a glance, so that I can quickly assess gateway health and usage.

**Acceptance Criteria:**
- AC-1: The dashboard displays: total requests (last 24h), error rate (last 24h), total cost (current month), active service accounts, top 5 models by request count, top 5 service accounts by cost.
- AC-2: Data refreshes automatically every 60 seconds.
- AC-3: Time range selector allows: last 1h, 6h, 24h, 7d, 30d.

#### FR-12.2: Request Explorer

**User Story:** As a platform engineer, I want to search and inspect individual requests, so that I can debug issues reported by application teams.

**Acceptance Criteria:**
- AC-1: Search by: request_id, service_account, model, provider, status, time range.
- AC-2: Results show: request_id, timestamp, service_account, model, provider, status, latency, tokens, cost.
- AC-3: Clicking a request shows full detail: request headers (redacted sensitive values), policy evaluation result, routing decision, retry attempts, provider response metadata.
- AC-4: Results are paginated (cursor-based, 50 per page).

#### FR-12.3: Service Account and Key Management UI

**User Story:** As a platform engineer, I want to manage service accounts and keys through the web UI, so that I do not need to use the API directly for routine operations.

**Acceptance Criteria:**
- AC-1: Create, view, edit, suspend/activate service accounts.
- AC-2: Register and revoke keys with a copy-to-clipboard flow for the key_id.
- AC-3: View key status, last used, and expiration.
- AC-4: Attach and detach policies.

#### FR-12.4: Policy and Route Management UI

**Acceptance Criteria:**
- AC-1: Create, view, edit, delete policies with form validation matching API constraints.
- AC-2: Create, view, edit, enable/disable routes with drag-and-drop priority ordering.
- AC-3: Route testing: a "Test Route" button that sends a synthetic request to validate provider connectivity.

#### FR-12.5: Usage and Cost Analytics

**Acceptance Criteria:**
- AC-1: Charts for: cost over time (line chart), cost by model (bar chart), cost by service account (bar chart), request volume over time (line chart), latency percentiles over time (line chart), error rate over time (line chart).
- AC-2: Filters: tenant, service account, model, provider, time range.
- AC-3: Export to CSV.

---

### 5.13 Python SDK

#### FR-13.1: Sync and Async Clients

**User Story:** As a backend developer, I want both synchronous and asynchronous Python clients, so that I can use the SDK in both traditional and async codebases.

**Acceptance Criteria:**
- AC-1: `GatewayClient` (sync, using `httpx`) and `AsyncGatewayClient` (async, using `httpx.AsyncClient`).
- AC-2: Both clients expose the same API surface: `client.chat.completions.create(...)`, `client.responses.create(...)`, `client.embeddings.create(...)`, `client.models.list()`.
- AC-3: Constructor takes: `gateway_url`, `service_account_id`, `key_id`, `private_key` (Ed25519 private key in PEM format or raw bytes).
- AC-4: The client auto-computes all signing headers (X-Timestamp, X-Nonce, X-Body-SHA256, X-Signature) transparently.

#### FR-13.2: Streaming Support

**User Story:** As a backend developer, I want to iterate over streaming responses, so that I can display tokens as they arrive.

**Acceptance Criteria:**
- AC-1: When `stream=True`, `create()` returns an iterator (sync) or async iterator (async).
- AC-2: Each yielded item is a parsed delta object.
- AC-3: The iterator properly handles SSE parsing, including multi-line data fields and `[DONE]` sentinel.
- AC-4: Context manager support for proper resource cleanup: `with client.chat.completions.create(..., stream=True) as stream:`.

#### FR-13.3: Retry Logic

**User Story:** As a backend developer, I want the SDK to retry transient failures automatically, so that my application is resilient to brief network issues.

**Acceptance Criteria:**
- AC-1: The SDK retries on: connection errors, HTTP 429 (respecting Retry-After), HTTP 500, HTTP 502, HTTP 503.
- AC-2: Default: 3 retries with exponential backoff (1s, 2s, 4s) plus jitter.
- AC-3: Retry behavior is configurable: `max_retries`, `backoff_factor`, `retry_on_status`.
- AC-4: Non-streaming requests are fully retried; streaming requests are NOT retried after the first byte is received (to avoid duplicate output).

#### FR-13.4: Error Handling

**Acceptance Criteria:**
- AC-1: Custom exception hierarchy: `GatewayError` (base), `AuthenticationError`, `PolicyDeniedError`, `RateLimitError`, `BudgetExhaustedError`, `ProviderError`, `TimeoutError`.
- AC-2: Each exception includes: `status_code`, `error_code`, `message`, `request_id`.
- AC-3: `RateLimitError` includes `retry_after` attribute.

---

## 6. Non-Functional Requirements

### 6.1 Performance

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| Gateway-added latency (non-streaming, p50) | < 10ms | OpenTelemetry span: gateway.request minus provider.call |
| Gateway-added latency (non-streaming, p99) | < 50ms | Same as above |
| Gateway-added latency (streaming, time-to-first-byte overhead) | < 5ms | Time from provider first byte to client first byte |
| Per-chunk relay latency (streaming) | < 1ms | Measured at the streaming relay layer |
| Auth verification latency (p99) | < 5ms | Includes Redis nonce check and Ed25519 verify |
| Policy evaluation latency (p99) | < 2ms | In-memory policy cache hit path |
| Throughput (single instance) | >= 5,000 requests/second | For non-streaming, small payload requests |
| Throughput (single instance, streaming) | >= 2,000 concurrent streams | Simultaneous SSE connections |
| Request body parsing (10 KB payload) | < 1ms | serde_json deserialization |

**Rationale:** Helicone (Rust-based) achieves approximately 50ms total overhead; our target of <50ms p99 gateway overhead (excluding provider) is achievable with Rust/Axum/Tokio. Axum benchmarks show sub-millisecond routing with memory usage of 12-20MB. The choice of Rust eliminates GC pauses that cause tail latency spikes in Go/Java gateways (source: Rust Web Frameworks Benchmark 2025, markaicode.com).

### 6.2 Scalability

| Dimension | Target | Approach |
|-----------|--------|----------|
| Horizontal scaling | Linear throughput increase | Stateless gateway instances behind a load balancer; all state in PostgreSQL/Redis |
| Maximum tenants | 10,000 per deployment | Cell-based architecture for larger scale (future) |
| Maximum service accounts | 100,000 per deployment | Indexed lookups, cached hot paths |
| Maximum concurrent connections | 50,000 per instance | Tokio async runtime, OS tuning (ulimit) |
| Usage events write throughput | 10,000 events/second per instance | Async batch inserts to PostgreSQL |
| Redis operations | 100,000 ops/second per instance | Connection pooling, pipelining |
| PostgreSQL connection pool | 20-50 connections per instance | Configurable via `sqlx` pool settings |

**Rationale:** Multi-tenant SaaS best practices in 2025 recommend the hybrid silo/pool model: shared infrastructure with logical tenant isolation via database-level tenant_id scoping. For ultra-large scale (10,000+ tenants), cell-based architecture partitions tenants into independent infrastructure units (source: isitdev.com, 2025).

### 6.3 Reliability

| Metric | Target |
|--------|--------|
| Gateway availability | 99.95% (26.3 minutes downtime/year) |
| Effective provider availability (with fallback) | 99.99% (when 2+ providers configured) |
| Data durability (usage events) | 99.99% (buffered in memory if DB is temporarily unavailable) |
| Mean time to failover (provider) | < 5 seconds |
| Recovery time after gateway restart | < 10 seconds (warm-up: load caches from DB) |

**Resilience patterns (layered, per industry best practice):**
1. **Rate limiting** (outermost): Prevents resource exhaustion.
2. **Timeouts**: No request hangs indefinitely.
3. **Retries with exponential backoff and jitter**: Handles transient failures.
4. **Circuit breaker** (Phase 2): Detects sustained provider failures, fast-fails requests to a tripped provider, periodically probes for recovery. States: Closed (normal) -> Open (after N failures in M seconds) -> Half-Open (probe). Configurable: failure_threshold (default: 5), window_seconds (default: 60), recovery_probe_interval_seconds (default: 30).
5. **Bulkhead isolation**: Per-provider connection pools prevent one failing provider from exhausting all connections.

(Source: Zuplo API Gateway Resilience Guide, 2025; AWS Advanced Multi-AZ Resilience Patterns)

### 6.4 Observability

#### 6.4.1 Distributed Tracing (OpenTelemetry)

| Requirement | Detail |
|-------------|--------|
| Protocol | OTLP (gRPC and HTTP) export |
| Trace propagation | W3C Trace Context (traceparent, tracestate) |
| Span hierarchy | `gateway.request` -> `auth.verify` -> `policy.evaluate` -> `routing.resolve` -> `provider.call` -> `streaming.relay` -> `usage.persist` |
| Attributes (per OTel GenAI Semantic Conventions v1.37+) | `gen_ai.system`, `gen_ai.request.model`, `gen_ai.response.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.request.max_tokens`, `gen_ai.request.temperature`, `gen_ai.response.finish_reasons` |
| Sampling | Configurable: default 100% for errors, 10% for success (tail-based sampling at collector recommended) |

**Rationale:** 85% of production LLM users plan for LLM observability, and the OpenTelemetry GenAI Semantic Conventions (stable as of v1.37) are the emerging industry standard. Datadog, Elastic, and others now natively ingest these conventions (source: OpenTelemetry blog, Datadog blog, 2025). The gateway's Rust implementation uses the `tracing` crate with `tracing-opentelemetry` bridge.

#### 6.4.2 Metrics (Prometheus / OpenTelemetry Metrics)

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `gateway_requests_total` | Counter | tenant_id, service_account_id, model, provider, status, method | Total requests processed |
| `gateway_request_duration_seconds` | Histogram | tenant_id, model, provider, status | End-to-end gateway latency |
| `gateway_provider_duration_seconds` | Histogram | provider, model, status | Provider call latency |
| `gateway_tokens_total` | Counter | tenant_id, model, provider, direction(input/output) | Token counts |
| `gateway_cost_dollars` | Counter | tenant_id, service_account_id, model, provider | Estimated cost in USD |
| `gateway_auth_failures_total` | Counter | reason | Authentication failures |
| `gateway_policy_denials_total` | Counter | tenant_id, reason | Policy denials |
| `gateway_rate_limit_hits_total` | Counter | tenant_id, service_account_id, level | Rate limit triggers |
| `gateway_provider_errors_total` | Counter | provider, error_type | Provider errors |
| `gateway_retries_total` | Counter | provider, model | Retry attempts |
| `gateway_fallbacks_total` | Counter | from_provider, to_provider, model | Fallback events |
| `gateway_active_streams` | Gauge | provider | Currently active streaming connections |
| `gateway_usage_buffer_size` | Gauge | | Buffered usage events pending write |
| `gateway_circuit_breaker_state` | Gauge | provider, state(closed/open/half_open) | Circuit breaker state |

#### 6.4.3 Structured Logging

| Requirement | Detail |
|-------------|--------|
| Format | JSON (one line per event) |
| Fields (every log line) | timestamp, level, message, request_id, trace_id, span_id, tenant_id, service_account_id |
| Sensitive data | Request/response bodies are NOT logged by default; opt-in via configuration with PII redaction |
| Log levels | ERROR: unrecoverable errors; WARN: security events, budget warnings, retries; INFO: request lifecycle (start, complete); DEBUG: policy evaluation details, routing decisions |
| Output | stdout (for container log collection) |
| Correlation | Every log line within a request shares the same `request_id` and `trace_id` |

#### 6.4.4 Health Check Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /healthz` | Liveness: returns 200 if the process is running |
| `GET /readyz` | Readiness: returns 200 if PostgreSQL and Redis are reachable and caches are warm |
| `GET /metrics` | Prometheus metrics endpoint |

### 6.5 Resource Consumption

| Resource | Target per Instance |
|----------|---------------------|
| Memory (idle) | < 50 MB |
| Memory (under load, 5k RPS) | < 500 MB |
| CPU (idle) | < 1% of 1 core |
| CPU (under load, 5k RPS) | < 2 cores |
| Disk | Minimal (logs to stdout, data in PostgreSQL/Redis) |
| Container image size | < 50 MB (static Rust binary + minimal base image) |

---

## 7. System Constraints and Assumptions

### 7.1 Constraints

| ID | Constraint | Rationale |
|----|-----------|-----------|
| C-1 | Backend must be written in Rust | Performance requirement; zero-cost abstractions and no GC pauses are critical for a proxy on the hot path |
| C-2 | Frontend must use SvelteKit + shadcn-svelte | Team skill set and existing toolchain |
| C-3 | Database must be PostgreSQL 15+ | Required for JSONB operators, advanced indexing, and proven multi-tenant support |
| C-4 | Cache/state store must be Redis 7+ | Required for Lua scripting (rate limiting), low-latency nonce checks, and pub/sub for cache invalidation |
| C-5 | API must be OpenAI-compatible | Industry standard; enables drop-in replacement for teams already using OpenAI SDK |
| C-6 | Auth must use Ed25519 asymmetric signing | Security requirement: no shared secrets transmitted; key compromise affects only one service account |
| C-7 | Self-hosted only (v1) | Enterprise security policy; no SaaS dependency for the gateway itself |
| C-8 | Single-region deployment (v1) | Multi-region introduces consistency challenges that are out of scope for MVP |

### 7.2 Assumptions

| ID | Assumption | Risk if Invalid |
|----|-----------|-----------------|
| A-1 | Provider APIs (OpenAI, Anthropic, Gemini) maintain backward compatibility for their current API versions | Adapters may break; mitigated by versioned adapter implementations |
| A-2 | Network latency between gateway and providers is < 100ms (same cloud region) | Provider call latency targets may not be met; mitigated by deploying gateway close to providers |
| A-3 | Redis is deployed as a cluster with < 1ms latency from gateway instances | Rate limiting and nonce checks depend on Redis performance |
| A-4 | Teams will adopt the Python SDK or use the OpenAI-compatible API surface | If teams need other language SDKs, additional development is required |
| A-5 | Peak load does not exceed 50,000 RPS across all gateway instances | Beyond this, PostgreSQL usage event writes may become a bottleneck; mitigated by async batching and future migration to a time-series DB |
| A-6 | Provider pricing changes are communicated in advance and updated manually | If pricing changes silently, cost estimates may be inaccurate for a short period |
| A-7 | NTP synchronization is maintained on all gateway instances (< 1s drift) | Timestamp validation in auth depends on clock accuracy |

---

## 8. Dependencies and Integrations

### 8.1 External Dependencies

| Dependency | Version | Purpose | Criticality |
|------------|---------|---------|-------------|
| OpenAI API | v1 (2024-stable) | LLM provider | High (primary provider for many teams) |
| Anthropic API | 2024-01-01+ | LLM provider | High |
| Google Gemini API | v1 | LLM provider | Medium |
| Azure OpenAI API | 2024-06-01+ | LLM provider (enterprise) | Medium |
| PostgreSQL | 15+ | Primary data store | Critical |
| Redis | 7+ | Cache, rate limiting, nonce store | Critical |
| OpenTelemetry Collector | 0.100+ | Telemetry export target | High (for observability) |

### 8.2 Internal Dependencies (Rust Crates)

| Crate | Purpose |
|-------|---------|
| `axum` | HTTP framework |
| `tokio` | Async runtime |
| `sqlx` | PostgreSQL async driver |
| `redis` (or `fred`) | Redis async client |
| `ed25519-dalek` | Ed25519 signature verification |
| `sha2` | SHA-256 hashing |
| `serde` / `serde_json` | JSON serialization |
| `tracing` + `tracing-opentelemetry` | Structured logging and OTel integration |
| `opentelemetry` + `opentelemetry-otlp` | OTel SDK |
| `reqwest` | HTTP client for provider calls |
| `tiktoken-rs` | Token counting for OpenAI models |
| `tower` + `tower-http` | Middleware (CORS, compression, request ID) |

### 8.3 SDK Dependencies (Python)

| Package | Purpose |
|---------|---------|
| `httpx` | HTTP client (sync + async) |
| `cryptography` (PyNaCl or ed25519) | Ed25519 signing |
| `pydantic` | Request/response models |

### 8.4 Frontend Dependencies

| Package | Purpose |
|---------|---------|
| `SvelteKit` | Application framework |
| `shadcn-svelte` | UI component library |
| `TypeScript` | Type safety |
| `chart.js` or `Apache ECharts` | Usage analytics charts |

---

## 9. Data Requirements

### 9.1 Data Model (Entity Overview)

```
tenants
  +-- service_accounts
  |     +-- service_account_keys
  |     +-- service_account_policy_bindings
  |     +-- usage_events
  +-- policies
  +-- budgets
  +-- provider_routes
  +-- pricing_rules (new)
  +-- audit_events
```

### 9.2 Key Tables and Relationships

| Table | Primary Key | Key Foreign Keys | Notes |
|-------|------------|------------------|-------|
| tenants | id (UUID) | -- | Logical isolation boundary |
| service_accounts | id (UUID) | tenant_id -> tenants.id | One per workload identity |
| service_account_keys | id (UUID) | service_account_id -> service_accounts.id | Multiple per SA for rotation |
| policies | id (UUID) | tenant_id -> tenants.id | Reusable across SAs |
| service_account_policy_bindings | id (UUID) | service_account_id, policy_id | Many-to-many |
| provider_routes | id (UUID) | tenant_id -> tenants.id (nullable for global) | Model alias -> provider mapping |
| budgets | id (UUID) | tenant_id, service_account_id (nullable) | Daily/monthly spend caps |
| usage_events | id (UUID) | tenant_id, service_account_id, provider_route_id | Append-only, high-volume |
| audit_events | id (UUID) | tenant_id (nullable) | Append-only, immutable |
| pricing_rules | id (UUID) | -- | Provider+model -> price mapping |

### 9.3 Data Volume Estimates (Per Year, Medium Deployment)

| Table | Estimated Row Count | Row Size | Total Size |
|-------|-------------------|----------|------------|
| tenants | 100 | ~200 B | ~20 KB |
| service_accounts | 5,000 | ~300 B | ~1.5 MB |
| service_account_keys | 15,000 | ~500 B | ~7.5 MB |
| policies | 1,000 | ~1 KB | ~1 MB |
| provider_routes | 2,000 | ~400 B | ~800 KB |
| usage_events | 500,000,000 (500M) | ~300 B | ~150 GB |
| audit_events | 500,000 | ~500 B | ~250 MB |

### 9.4 Data Retention Policy

| Data Type | Default Retention | Configurable | Archive Strategy |
|-----------|-------------------|--------------|-----------------|
| usage_events | 90 days hot (PostgreSQL) | Yes | Partition by month; archive to object storage (Parquet format) |
| audit_events | 365 days hot | Yes | Archive to object storage |
| Redis nonces | 300 seconds | Yes (tied to timestamp window) | Auto-expire (TTL) |
| Redis rate limit counters | 60 seconds | Yes (tied to window size) | Auto-expire (TTL) |
| Redis sessions | 8 hours | Yes | Auto-expire (TTL) |

### 9.5 Data Partitioning Strategy

- **usage_events**: Partitioned by `created_at` (monthly range partitions) for efficient time-range queries and partition-level drops during archival.
- **audit_events**: Partitioned by `created_at` (monthly range partitions).
- **Indexing**: All foreign key columns indexed. Additional indexes on frequently queried columns: `usage_events(model_alias)`, `usage_events(provider)`, `usage_events(created_at DESC)`.

---

## 10. Security Requirements

This section is informed by the OWASP API Security Top 10 (2023 edition) and industry best practices for API gateway security.

### 10.1 OWASP API1:2023 -- Broken Object Level Authorization

| Mitigation | Implementation |
|------------|----------------|
| Tenant isolation | Every database query includes `tenant_id` in the WHERE clause; no cross-tenant data access |
| Service account scoping | Data plane requests can only access resources within the authenticated service account's tenant |
| Admin API authorization | Admin users can only manage resources within their authorized tenant scope (super-admin role required for cross-tenant) |
| Object ID validation | All object IDs (UUID) are validated for format AND ownership before access |

### 10.2 OWASP API2:2023 -- Broken Authentication

| Mitigation | Implementation |
|------------|----------------|
| Cryptographic auth | Ed25519 asymmetric signing (no shared secrets, no bearer tokens on the data plane) |
| Key expiration | Keys have optional `expires_at`; expired keys are automatically rejected |
| Account lockout | After 10 consecutive auth failures from the same IP within 5 minutes, the IP is temporarily blocked (configurable) |
| Constant-time comparison | Signature verification uses constant-time byte comparison to prevent timing attacks |
| Admin auth | Bcrypt-hashed API keys (cost factor 12); session tokens in Redis with strict TTL |

### 10.3 OWASP API3:2023 -- Broken Object Property Level Authorization

| Mitigation | Implementation |
|------------|----------------|
| Response filtering | Admin API responses exclude internal fields (e.g., public_key_pem is returned on create only, never on list/get) |
| Input validation | Only whitelisted fields are accepted for create/update operations; extra fields are rejected (strict mode) |
| Partial updates | PATCH operations validate each field individually against the caller's role permissions |

### 10.4 OWASP API4:2023 -- Unrestricted Resource Consumption

| Mitigation | Implementation |
|------------|----------------|
| Rate limiting | Multi-level sliding window rate limiting (global, tenant, service account) |
| Concurrency limiting | Max concurrent in-flight requests per service account |
| Request body size | Configurable max body size (default: 10 MB) |
| Token limits | Policy-enforced max_input_tokens and max_output_tokens |
| Budget caps | Daily and monthly budget enforcement with automatic request blocking |
| Connection limits | Per-client connection limit at the load balancer level |
| Query complexity | Admin API list endpoints have mandatory pagination with max page size |

### 10.5 OWASP API5:2023 -- Broken Function Level Authorization

| Mitigation | Implementation |
|------------|----------------|
| API separation | Data plane and admin plane are separate API surface areas with different auth mechanisms |
| RBAC | Admin API enforces role-based access (admin, viewer); function-level checks on every endpoint |
| No admin via data plane | Data plane endpoints cannot perform any administrative functions |

### 10.6 OWASP API6:2023 -- Unrestricted Access to Sensitive Business Flows

| Mitigation | Implementation |
|------------|----------------|
| Rate limiting on admin ops | Administrative operations (key creation, policy changes) are rate-limited separately |
| Audit logging | All administrative actions are logged with actor identity |
| Anomaly detection (Phase 3) | Detect unusual patterns: sudden spike in requests from a service account, unusual model usage |

### 10.7 OWASP API7:2023 -- Server Side Request Forgery (SSRF)

| Mitigation | Implementation |
|------------|----------------|
| Provider URL allowlist | Provider base URLs are configured by admins only; the gateway never constructs URLs from user input |
| No URL parameters from clients | Clients specify model aliases, not URLs; the routing engine resolves aliases to pre-configured provider endpoints |
| Internal network blocking | Provider adapter HTTP client is configured to block requests to private IP ranges (10.x, 172.16.x, 192.168.x, 127.x, ::1) |

### 10.8 OWASP API8:2023 -- Security Misconfiguration

| Mitigation | Implementation |
|------------|----------------|
| Secure defaults | TLS required, 0-RTT disabled, CORS restricted, verbose errors disabled in production |
| Security headers | Strict-Transport-Security, X-Content-Type-Options, X-Frame-Options, Content-Security-Policy on all responses |
| Error messages | Production error responses include error codes and request_id but never stack traces or internal details |
| Configuration validation | Gateway validates configuration at startup and refuses to start with insecure settings (e.g., TLS disabled without explicit override flag) |
| Dependency scanning | CI pipeline includes `cargo audit` for known Rust crate vulnerabilities |

### 10.9 OWASP API9:2023 -- Improper Inventory Management

| Mitigation | Implementation |
|------------|----------------|
| API versioning | All endpoints are versioned (`/v1/`); deprecated endpoints return Deprecation header |
| OpenAPI spec | Machine-readable OpenAPI 3.1 spec is auto-generated and served at `/openapi.json` |
| Model inventory | `GET /v1/models` returns only models the authenticated service account is authorized to access |
| Admin inventory | Admin API exposes `GET /admin/v1/routes` for full route inventory |

### 10.10 OWASP API10:2023 -- Unsafe Consumption of APIs

| Mitigation | Implementation |
|------------|----------------|
| Provider response validation | Provider responses are validated against expected schema before forwarding to clients |
| Timeout enforcement | All provider calls have strict timeouts; no unbounded waits |
| TLS to providers | All outbound provider connections use TLS 1.2+ with certificate verification |
| Error isolation | Provider errors are normalized; raw provider error messages are not passed through by default |
| Response size limits | Provider responses exceeding 100 MB are truncated and an error is returned |

### 10.11 Additional Security Measures

| Measure | Detail |
|---------|--------|
| Secrets management | Provider API keys are stored encrypted at rest (AES-256-GCM) in PostgreSQL with a master key from environment variable or KMS |
| Key material handling | Ed25519 private keys never touch the gateway; only public keys are stored. Private keys exist only on the client side |
| PII protection | Request/response bodies are not logged or stored by default; opt-in logging redacts common PII patterns |
| Dependency pinning | All crate versions are pinned in Cargo.lock; npm packages pinned in package-lock.json |
| Container security | Distroless or scratch-based container image; non-root user; read-only filesystem |

---

## 11. Compliance and Regulatory Considerations

### 11.1 EU AI Act (Effective 2025)

The EU AI Act classifies AI systems by risk level. An LLM gateway is an infrastructure component, not an AI system itself, but it must support compliance for the AI systems it serves:

| Requirement | Gateway Support |
|-------------|----------------|
| Transparency obligations | Usage logging provides a record of which AI models were used, when, and by whom |
| Risk management | Policy engine can restrict access to high-risk model capabilities (e.g., block certain models in certain environments) |
| Human oversight | Audit trail enables human review of AI system usage patterns |
| Data governance | Request/response logging (when enabled) supports data lineage tracking |

**Note:** Fines for EU AI Act violations can reach 35 million EUR or 7% of global annual turnover (source: EU AI Act, 2025). The gateway helps organizations demonstrate compliance but does not itself classify AI risk levels.

### 11.2 SOC 2 Type II Readiness

| Control | Gateway Feature |
|---------|-----------------|
| Access control | Ed25519 auth, RBAC for admin, policy engine |
| Audit logging | Immutable audit trail for all administrative actions |
| Change management | Audit events capture before/after state for configuration changes |
| Availability monitoring | Health check endpoints, uptime metrics, SLO tracking |
| Incident response | Request tracing with correlation IDs for rapid diagnosis |

### 11.3 GDPR and Data Privacy

| Consideration | Implementation |
|---------------|----------------|
| Data minimization | Gateway does not store request/response bodies by default |
| Right to erasure | Usage events can be purged by tenant_id (admin operation) |
| Data processing records | Usage events serve as processing activity records |
| Cross-border data transfer | Route region configuration allows restricting which providers (and their data residency) are used for a tenant |
| PII in prompts | Content filtering (Phase 2) can detect and block PII in prompts before they reach providers |

### 11.4 Industry Standards

| Standard | Relevance |
|----------|-----------|
| NIST AI 600-1 (AI RMF) | Gateway supports AI risk management through access controls, monitoring, and audit trails |
| ISO 27001 | Gateway security controls align with Annex A requirements for access control, cryptography, operations security, and communications security |
| PCI DSS (if applicable) | If LLM requests handle payment data, policy engine can enforce model restrictions and logging requirements |

---

## 12. MVP Scope vs Future Phases

### 12.1 Phase 1: MVP (Months 1-3)

**Goal:** Core gateway functionality sufficient for a single team to adopt in production.

| Component | Features Included |
|-----------|-------------------|
| Data Plane API | POST /v1/chat/completions (stream + non-stream), POST /v1/responses, POST /v1/embeddings, GET /v1/models |
| Auth | Ed25519 signing verification, timestamp validation, nonce replay protection, body integrity check |
| Service Accounts | Create, list, get, suspend; register/revoke keys |
| Policy Engine | Model access control (allow/deny lists), token limits, rate limiting (RPM), feature flags (streaming, tools) |
| Routing | Model alias resolution, priority-based routing, basic retry (same provider), basic fallback (next priority provider) |
| Provider Adapters | OpenAI-compatible (including vLLM), Anthropic |
| Streaming | SSE relay, client disconnect handling, usage finalization |
| Usage Metering | Per-request logging to PostgreSQL, basic cost estimation |
| Rate Limiting | Per-service-account sliding window (Redis) |
| Audit Logging | Administrative actions logged to PostgreSQL |
| Observability | Structured JSON logging, basic Prometheus metrics, request_id/trace_id propagation |
| Python SDK | Sync + async clients, request signing, streaming, basic retry |
| Admin API | CRUD for tenants, service accounts, keys, policies, routes |

**Not in MVP:** Admin Console UI, budget enforcement, concurrency limiting, circuit breaker, Gemini/Azure adapters, OpenTelemetry tracing export, caching, content filtering, analytics dashboards.

### 12.2 Phase 2: Production Hardening (Months 4-6)

| Component | Features Added |
|-----------|----------------|
| Admin Console | Dashboard, request explorer, service account management UI, policy/route management UI |
| Budget Enforcement | Daily/monthly budgets, automated blocking, warning alerts |
| Concurrency Limiting | Per-service-account concurrent request limits |
| Provider Adapters | Google Gemini, Azure OpenAI |
| Circuit Breaker | Per-provider circuit breaker with configurable thresholds |
| OpenTelemetry | Full distributed tracing with OTel GenAI Semantic Conventions, OTLP export |
| Rate Limiting | Multi-level (global, tenant, service account) |
| Usage Analytics | Aggregation queries, CSV export |
| Pricing Table | Admin-managed pricing rules, accurate cost computation |
| Health-Aware Routing | Provider health tracking in Redis, automatic deprioritization of unhealthy providers |

### 12.3 Phase 3: Advanced Features (Months 7-12)

| Component | Features Added |
|-----------|----------------|
| Semantic Caching | Vector similarity-based caching for repeated/similar prompts (cache hit in ~5ms vs 2000ms+ provider round-trip); exact-match caching for embeddings and deterministic prompts |
| AI Safety Guardrails | Input validation (prompt injection detection, PII detection), output filtering (toxicity scoring, content policy enforcement). Multi-layered: input filter -> model constraints -> output filter (source: AI Guardrails Production Guide, Iterathon 2026) |
| Anomaly Detection | Unusual usage pattern detection (spike in requests, unusual model access, cost anomalies), automated alerts |
| Advanced Routing | Cost-aware routing (route to cheapest provider meeting quality requirements), latency-aware routing (route to fastest provider), weighted load balancing |
| Content Logging | Optional full request/response body logging with PII redaction, stored in object storage |
| Multi-Region Support | Cross-region deployment with regional route preferences |
| Additional SDKs | TypeScript/JavaScript, Go, Java SDK |
| Prompt Management | Prompt templates, version management, A/B testing |
| Agent Control Plane | Governance for AI agent workflows: independent visibility and enforcement sitting outside the agent's execution loop (source: AI Guardrails Production Guide, 2026) |

### 12.4 Phase Boundary Rules

1. **No Phase 2/3 features may delay Phase 1 delivery.** If a Phase 1 feature threatens the timeline, it moves to Phase 2 -- not the other way around.
2. **Architecture must not preclude future phases.** Phase 1 code must be designed with extension points (trait-based adapters, pluggable policy evaluators, middleware chains).
3. **Phase transitions require:** all Phase N acceptance criteria passing, load test at 2x expected peak, security review sign-off.

---

## 13. Success Metrics and KPIs

### 13.1 Adoption Metrics

| KPI | Target (6 months) | Measurement |
|-----|-------------------|-------------|
| Services onboarded | >= 80% of LLM-consuming services | Count of active service accounts |
| Request coverage | >= 95% of org LLM requests via gateway | Compare gateway usage_events vs provider invoice totals |
| SDK adoption | >= 5 teams using Python SDK | SDK download/import telemetry |
| Admin console daily active users | >= 3 | Session analytics |

### 13.2 Reliability Metrics

| KPI | Target | Measurement |
|-----|--------|-------------|
| Gateway uptime | >= 99.95% | Health check monitoring |
| Request success rate (gateway-attributable) | >= 99.99% | Requests that fail due to gateway bugs, not provider/client errors |
| Provider failover success rate | >= 99% | Successful fallbacks / total fallback attempts |
| Mean time to detect (MTTD) gateway issues | < 5 minutes | Alert firing latency |

### 13.3 Performance Metrics

| KPI | Target | Measurement |
|-----|--------|-------------|
| Gateway overhead p50 | < 10ms | OTel traces |
| Gateway overhead p99 | < 50ms | OTel traces |
| Streaming TTFB overhead | < 5ms | OTel traces |

### 13.4 Security Metrics

| KPI | Target | Measurement |
|-----|--------|-------------|
| Unmanaged API keys | 0 | Audit: no direct provider API calls from application code |
| Key rotation compliance | 100% of keys rotated within 90 days | Key age monitoring |
| Auth failure rate | < 0.1% of total requests | Metrics |
| Mean time to revoke compromised key | < 15 minutes | Incident response tracking |

### 13.5 Cost Management Metrics

| KPI | Target | Measurement |
|-----|--------|-------------|
| Cost attribution accuracy | >= 98% (gateway estimate vs provider invoice) | Monthly reconciliation |
| Budget breach incidents | 0 (no spend exceeding set budgets) | Budget enforcement logs |
| Cost savings from optimization | >= 15% reduction in first 6 months | Compare pre/post gateway LLM spend |

---

## 14. Risk Register

| ID | Risk | Likelihood | Impact | Mitigation | Owner |
|----|------|-----------|--------|------------|-------|
| R-1 | **Provider API breaking changes** break adapters | Medium | High | Version-pinned adapters, integration test suite running daily against live provider APIs, adapter abstraction layer for rapid updates | Platform Eng |
| R-2 | **Multi-provider response inconsistency** causes application bugs | High | Medium | Strict output normalization in adapters; documented differences in provider capabilities (e.g., tool use support varies); integration test matrix | Platform Eng |
| R-3 | **Cost miscalculation** due to stale pricing or unreported reasoning tokens | Medium | High | Automated pricing table updates via provider API scraping; reconciliation reports comparing estimates vs invoices; alert on >5% deviation | FinOps |
| R-4 | **Key management complexity** leads to operational errors or lockouts | Medium | High | SDK handles signing transparently; key rotation guide with zero-downtime workflow; maximum keys per SA limit; key expiry alerts 14 days in advance | Security Eng |
| R-5 | **Redis failure** disables rate limiting and nonce protection | Low | Critical | Redis Sentinel or Cluster for HA; degraded mode: if Redis is unreachable, reject requests (fail-closed for security) with alert; fallback to in-memory rate limiting for brief outages (< 30s) | Platform Eng |
| R-6 | **PostgreSQL failure** prevents usage recording and admin operations | Low | High | PostgreSQL HA (streaming replication + failover); usage events buffered in memory (up to 10,000) during outage; admin operations return 503 | Platform Eng |
| R-7 | **Performance regression** in gateway code increases latency beyond targets | Medium | High | Automated benchmark suite in CI; reject PRs that regress p99 latency by >10%; load testing before each release | Platform Eng |
| R-8 | **Streaming complexity** causes data corruption or token miscounting | Medium | Medium | Extensive integration tests for streaming with each provider; fuzz testing on SSE parsing; reconciliation of stream token counts vs provider usage API | Platform Eng |
| R-9 | **SDK adoption resistance** from teams preferring direct provider calls | Medium | High | Drop-in compatibility (change only base URL); demonstrate value (cost visibility, automatic retries); executive mandate for security compliance | Platform Eng + Engineering Leadership |
| R-10 | **Single point of failure** if all traffic routes through the gateway | Medium | Critical | Horizontal scaling with health-check-based load balancing; rolling deployments; graceful shutdown (drain connections); documented manual bypass procedure for emergency | Platform Eng |
| R-11 | **Prompt injection attacks** via crafted inputs bypass safety controls | High | Medium (Phase 3) | Phase 3: input validation guardrails with prompt injection detection; Phase 1 mitigation: policy engine restricts model access and token limits | Security Eng |
| R-12 | **Scope creep** delays MVP delivery | High | High | Strict phase boundary rules (Section 12.4); weekly scope review; any feature not in Phase 1 table is deferred | Product Owner |
| R-13 | **Clock skew** between gateway instances causes false auth rejections | Low | Medium | NTP mandatory on all instances; 5-minute timestamp window provides buffer; monitoring for clock drift with alert at >2s deviation | Platform Eng |

---

## 15. Glossary

| Term | Definition |
|------|-----------|
| **Adapter** | A module that translates between the gateway's normalized request/response format and a specific LLM provider's API format |
| **Asymmetric Signing** | A cryptographic method using a private key (held by the client) to sign requests and a corresponding public key (held by the gateway) to verify them, eliminating shared secret transmission |
| **Budget** | A configurable spending limit (daily, monthly, or custom period) that the gateway enforces by rejecting requests when the limit is reached |
| **Circuit Breaker** | A resilience pattern that detects sustained failures to a provider and temporarily stops sending requests, allowing the provider to recover |
| **Control Plane** | The administrative API and UI used to configure the gateway (tenants, service accounts, policies, routes) -- as opposed to the data plane which handles runtime LLM requests |
| **Correlation ID** | A unique identifier (request_id or trace_id) that links all log entries, spans, and events related to a single request for end-to-end tracing |
| **Data Plane** | The runtime API surface that handles LLM requests from client applications (chat completions, embeddings, etc.) |
| **Ed25519** | An elliptic curve digital signature algorithm (RFC 8032) used for request authentication. Provides 128-bit security with fast signing and verification |
| **Fallback** | When the primary provider route fails, the gateway automatically attempts the next provider in the priority chain |
| **Gateway Overhead** | The additional latency introduced by the gateway's processing (auth, policy, routing, logging) -- excluding the time spent waiting for the provider's response |
| **GenAI Semantic Conventions** | An OpenTelemetry specification (v1.37+) defining standard attribute names for AI/LLM telemetry (e.g., gen_ai.usage.input_tokens) |
| **Model Alias** | A stable, provider-agnostic name (e.g., "smart-large") that maps to one or more provider-specific models (e.g., "gpt-4o", "claude-sonnet-4") via routing configuration |
| **Nonce** | A unique random value included in each signed request to prevent replay attacks. Each nonce is stored in Redis for the duration of the timestamp window |
| **OTLP** | OpenTelemetry Protocol -- the standard protocol for exporting telemetry data (traces, metrics, logs) to observability backends |
| **Policy** | A set of rules (model access, token limits, rate limits, budget caps, feature flags) attached to a service account to control its LLM access |
| **Provider** | An external LLM API service (OpenAI, Anthropic, Google Gemini, Azure OpenAI) or a self-hosted inference server (vLLM) |
| **Provider Route** | A configuration entry mapping a model alias to a specific provider and model name, with priority, timeout, and retry settings |
| **RBAC** | Role-Based Access Control -- restricting admin API operations based on the caller's assigned role (admin, viewer) |
| **Semantic Caching** | A caching strategy that matches requests by meaning (vector similarity) rather than exact text, enabling cache hits for paraphrased but semantically equivalent prompts |
| **Service Account** | A workload identity representing an application or service that consumes LLMs through the gateway. Each service account belongs to a tenant and can have multiple signing keys |
| **Sliding Window** | A rate limiting algorithm that tracks request counts over a rolling time window (e.g., the last 60 seconds), as opposed to fixed windows which reset at boundaries |
| **SSE** | Server-Sent Events -- a standard for streaming data from server to client over HTTP. Used by LLM APIs for token-by-token streaming |
| **Tenant** | The top-level organizational unit in the gateway's multi-tenancy model. Typically represents a team, department, or business unit. All resources (service accounts, policies, routes) belong to a tenant |
| **TTFB** | Time to First Byte -- the duration between the gateway sending a request to the provider and receiving the first byte of the response. Critical for streaming latency |
| **Usage Event** | A database record capturing the metadata of a completed LLM request: tokens used, cost, latency, provider, model, status |

---

## 16. References

### Industry and Architecture Research

- [Top 5 LLM Gateways in 2025 -- Helicone](https://www.helicone.ai/blog/top-llm-gateways-comparison-2025) -- Comprehensive comparison of LLM gateway architectures including performance benchmarks (Rust-based Helicone at ~50ms overhead)
- [Top 5 LLM Gateways in 2026 -- DEV Community](https://dev.to/varshithvhegde/top-5-llm-gateways-in-2026-a-deep-dive-comparison-for-production-teams-34d2) -- Updated comparison for 2026 production teams
- [Why Your AI App Needs an LLM API Gateway (2026) -- Ofox](https://ofox.ai/blog/why-llm-api-gateway-how-to-choose-2026/) -- Decision framework for LLM gateway selection
- [LiteLLM AI Gateway Documentation](https://docs.litellm.ai/docs/simple_proxy) -- Reference architecture for OpenAI-compatible proxy
- [LLM Cost Optimization in 2026: Routing, Caching, and Batching -- Mavik Labs](https://www.maviklabs.com/blog/llm-cost-optimization-2026) -- Cost reduction strategies achieving 47-80% savings
- [LLM Token Optimization: Cut Costs and Latency in 2026 -- Redis](https://redis.io/blog/llm-token-optimization-speed-up-apps/) -- Token optimization with caching strategies

### Security

- [OWASP API Security Top 10 (2023)](https://owasp.org/API-Security/editions/2023/en/0x11-t10/) -- Authoritative API security risk list
- [OWASP API Security Top 10 2023 Explained -- Salt Security](https://salt.security/blog/owasp-api-security-top-10-explained) -- Detailed explanation of each risk with mitigations
- [Best LLM Gateways for Security -- Pomerium](https://www.pomerium.com/blog/best-llm-gateways-in-2025) -- Security-focused gateway comparison

### Observability

- [AI Agent Observability -- Evolving Standards and Best Practices -- OpenTelemetry](https://opentelemetry.io/blog/2025/ai-agent-observability/) -- OTel standards for AI/LLM observability
- [An Introduction to Observability for LLM-based Applications -- OpenTelemetry](https://opentelemetry.io/blog/2024/llm-observability/) -- Foundation for LLM observability patterns
- [Datadog LLM Observability Supports OTel GenAI Semantic Conventions -- Datadog](https://www.datadoghq.com/blog/llm-otel-semantic-convention/) -- Industry adoption of OTel GenAI conventions
- [Can OpenTelemetry Save Observability in 2026 -- The New Stack](https://thenewstack.io/can-opentelemetry-save-observability-in-2026/) -- State of OTel ecosystem
- [Observability Trends for 2026: GenAI and OpenTelemetry -- Elastic](https://www.elastic.co/blog/2026-observability-trends-generative-ai-opentelemetry) -- Industry trends

### AI Safety and Guardrails

- [AI Guardrails Production Implementation Guide 2026 -- Iterathon](https://iterathon.tech/blog/ai-guardrails-production-implementation-guide-2026) -- Production-ready guardrail patterns
- [5 Best AI Guardrails Platforms Compared in 2026 -- Galileo](https://galileo.ai/blog/best-ai-guardrails-platforms) -- Guardrail platform comparison
- [Keep AI Interactions Secure with Guardrails in AI Gateway -- Cloudflare](https://blog.cloudflare.com/guardrails-in-ai-gateway/) -- Gateway-integrated guardrail patterns
- [LLM Guardrails: Strategies and Best Practices in 2025 -- Leanware](https://www.leanware.co/insights/llm-guardrails) -- Multi-layered defense approach

### Multi-Tenant Architecture

- [Multi-Tenant SaaS Architecture on Cloud (2025) -- isitdev](https://isitdev.com/multi-tenant-saas-architecture-cloud-2025/) -- Cell-based architecture for ultra-large scale
- [SaaS Multi-Tenant Architecture Design Patterns (2025) -- Zenn](https://zenn.dev/shineos/articles/saas-multi-tenant-architecture-2025) -- Hybrid silo/pool patterns
- [Multi-Tenant SaaS Authorization and API Access Control -- AWS](https://docs.aws.amazon.com/prescriptive-guidance/latest/saas-multitenant-api-access-authorization/introduction.html) -- Best practices for multi-tenant API authorization

### Performance and Rust

- [Rust Web Frameworks 2025: Axum vs Actix vs Rocket Benchmark -- Markaicode](https://markaicode.com/vs/rust-web-frameworks-in-2025-axum-vs-actix-vs-rocket-performance-benchmark/) -- Axum benchmark data
- [Axum HTTP Framework Documentation -- docs.rs](https://docs.rs/axum/latest/axum/) -- Axum reference
- [API Gateway Resilience and Fault Tolerance -- Zuplo](https://zuplo.com/learning-center/api-gateway-resilience-fault-tolerance) -- Layered resilience pattern reference

### Compliance

- [EU AI Act](https://digital-strategy.ec.europa.eu/en/policies/regulatory-framework-ai) -- Regulatory framework with fines up to 35M EUR / 7% turnover
- [NIST AI 600-1 (AI Risk Management Framework)](https://www.nist.gov/artificial-intelligence) -- US federal AI governance framework

---

*End of Document*
