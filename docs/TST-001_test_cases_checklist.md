# LLMSmartGate -- Test Cases & Checklist Document

> **Doc ID:** `TST-001`  
> **Version:** 1.0  
> **Date:** 2026-04-01  
> **Status:** Draft  
> **Classification:** Internal -- QA

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Senior QA Engineer |
| **Perspective** | Verification & validation -- prove the system works correctly and securely |
| **Primary Audience** | QA Engineers, SDET, Security Testers |
| **Secondary Audience** | Developers (unit test guidance), Product Owner (coverage confidence) |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| **TC-xxx** | Test Case ID -- unique, traceable to PRD requirements |
| **Priority: P0** | Blocker -- must pass before any release |
| **Priority: P1** | Critical -- must pass before production release |
| **Priority: P2** | Important -- should pass, acceptable to defer with risk note |
| **Priority: P3** | Nice-to-have -- edge case, defer if time-constrained |
| **Type: Normal** | Happy path -- expected user behavior |
| **Type: Abnormal** | Edge case, error path, attack vector, boundary condition |
| **[OWASP-APIx]** | Maps to OWASP API Security Top 10 (2023) item |
| **[OWASP-LLMx]** | Maps to OWASP LLM Top 10 (2025) item |
| **[PERF]** | Performance test -- requires load testing infrastructure |
| **[SEC]** | Security test -- may require specialized tools (ZAP, Burp) |

### Test Case Table Format

| Column | Description |
|--------|-------------|
| ID | Unique identifier (e.g., `TC-AUTH-001`) |
| Category | Module being tested |
| Test Case | What is being verified |
| Pre-conditions | Setup required before execution |
| Steps | Numbered execution steps |
| Expected Result | Pass criteria |
| Priority | P0 / P1 / P2 / P3 |
| Type | Normal / Abnormal |

### Checklist Gate Reviews (Part 6)

| Symbol | Meaning |
|--------|---------|
| ✅ | Verified and passing |
| ⚠️ | Known issue -- risk accepted with documented justification |
| ❌ | Blocking -- must resolve before proceeding |

### Document Relationships

```
PRD-001 (US-xxx, AC-xxx)     -- Requirements being tested
  ├── ARC-001                 -- Standards compliance tests
  ├── HLD-001                 -- Integration flow tests
  ├── LLD-BE-001              -- Unit & integration test targets
  ├── LLD-FE-001              -- Component & E2E test targets
  └── TST-001 (this)          -- Test execution & verification
```

---

## Table of Contents

- [Part 1: Test Strategy](#part-1-test-strategy)
  - [1.1 Test Levels](#11-test-levels)
  - [1.2 Test Environment Requirements](#12-test-environment-requirements)
  - [1.3 Test Data Strategy](#13-test-data-strategy)
  - [1.4 Test Automation Framework Recommendations](#14-test-automation-framework-recommendations)
  - [1.5 Entry/Exit Criteria](#15-entryexit-criteria)
- [Part 2: Functional Test Cases](#part-2-functional-test-cases)
  - [2.1 Authentication Module](#21-authentication-module)
  - [2.2 API Layer -- Chat Completions](#22-api-layer----chat-completions)
  - [2.3 API Layer -- Streaming](#23-api-layer----streaming)
  - [2.4 API Layer -- Embeddings](#24-api-layer----embeddings)
  - [2.5 Policy Engine](#25-policy-engine)
  - [2.6 Routing Engine](#26-routing-engine)
  - [2.7 Rate Limiting](#27-rate-limiting)
  - [2.8 Usage & Budget](#28-usage--budget)
  - [2.9 Admin API -- Tenants](#29-admin-api----tenants)
  - [2.10 Admin API -- Service Accounts](#210-admin-api----service-accounts)
  - [2.11 Admin API -- Keys](#211-admin-api----keys)
  - [2.12 Admin API -- Policies](#212-admin-api----policies)
  - [2.13 Admin API -- Routes](#213-admin-api----routes)
  - [2.14 Audit Logging](#214-audit-logging)
- [Part 3: Non-Functional Test Cases](#part-3-non-functional-test-cases)
  - [3.1 Performance Tests](#31-performance-tests)
  - [3.2 Security Tests](#32-security-tests)
  - [3.3 Reliability Tests](#33-reliability-tests)
  - [3.4 Scalability Tests](#34-scalability-tests)
- [Part 4: Frontend (Admin Console) Test Cases](#part-4-frontend-admin-console-test-cases)
- [Part 5: SDK Test Cases](#part-5-sdk-test-cases)
- [Part 6: Checklists](#part-6-checklists)
  - [6.1 Pre-deployment Checklist](#61-pre-deployment-checklist)
  - [6.2 Security Review Checklist](#62-security-review-checklist)
  - [6.3 Performance Review Checklist](#63-performance-review-checklist)
  - [6.4 Observability Checklist](#64-observability-checklist)
  - [6.5 Release Readiness Checklist](#65-release-readiness-checklist)
- [Appendix A: Traceability Matrix](#appendix-a-traceability-matrix)
- [Appendix B: References](#appendix-b-references)

---

# Part 1: Test Strategy

## 1.1 Test Levels

### 1.1.1 Unit Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | Individual Rust functions, SvelteKit components, Python SDK methods |
| Owner         | Developers |
| Framework     | `cargo test` (Rust), Vitest (SvelteKit), pytest (Python SDK) |
| Coverage Goal | >= 80% line coverage on core modules (auth, policy, routing, metering) |
| Execution     | On every commit, pre-merge gate |
| Focus         | Pure logic: canonical string construction, signature verification, policy evaluation, cost calculation, token counting, JSON schema validation |

### 1.1.2 Integration Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | Module boundaries: API -> Auth -> Policy -> Routing -> Provider chain; Admin API -> PostgreSQL; Rate limiting -> Redis |
| Owner         | Developers + QA |
| Framework     | `cargo test` with testcontainers (PostgreSQL, Redis), httptest for HTTP mocking |
| Coverage Goal | All module interaction paths exercised |
| Execution     | On every PR merge to develop, nightly |
| Focus         | Database queries return expected results, Redis operations (nonce, rate limit) behave correctly, provider adapter request/response transformations |

### 1.1.3 System Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | Full gateway deployed with all dependencies (PostgreSQL, Redis, mock providers) |
| Owner         | QA |
| Framework     | k6 for HTTP scenarios, custom test harness using Python SDK |
| Coverage Goal | All API endpoints exercised end-to-end |
| Execution     | Per release candidate |
| Focus         | Complete request lifecycle from signed request to provider call to usage recording |

### 1.1.4 End-to-End Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | Admin Console (SvelteKit) + Admin API + Data Plane API + Python SDK |
| Owner         | QA |
| Framework     | Playwright (Admin Console), Python SDK integration tests |
| Coverage Goal | All critical user journeys |
| Execution     | Nightly, pre-release |
| Focus         | Platform engineer creates tenant -> creates SA -> registers key -> sets policy -> sets route -> SDK makes LLM call -> usage appears in dashboard |

### 1.1.5 Performance Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | Data Plane API under load |
| Owner         | QA + SRE |
| Framework     | Grafana k6 (v1.0+), with TypeScript test scripts |
| Coverage Goal | Latency SLOs validated at 100, 1000, 10000 concurrent users |
| Execution     | Weekly, pre-release |
| Focus         | p95 overhead < 200ms, streaming TTFT, throughput saturation point |

### 1.1.6 Security Tests

| Attribute     | Detail |
|---------------|--------|
| Scope         | All external-facing APIs (Data Plane + Admin), auth subsystem, TLS |
| Owner         | Security + QA |
| Framework     | OWASP ZAP, custom fuzzing scripts, manual penetration testing |
| Coverage Goal | OWASP API Security Top 10 (2023) + OWASP Top 10 for LLM Applications (2025) |
| Execution     | Per release, quarterly pen test |
| Focus         | Authentication bypass, replay attacks, injection, authorization flaws |

---

## 1.2 Test Environment Requirements

### 1.2.1 Environment Matrix

| Environment | Purpose | Infrastructure |
|-------------|---------|---------------|
| Local Dev   | Unit + Integration | Docker Compose: PostgreSQL 16, Redis 7, mock provider stubs |
| CI          | Automated gates | GitHub Actions / GitLab CI with testcontainers, ephemeral DBs |
| Staging     | System + E2E + Performance | Kubernetes namespace mirroring production topology |
| Security    | Penetration testing | Isolated environment, real provider sandboxes |
| Production  | Smoke tests only | Canary-based deployment with synthetic traffic |

### 1.2.2 Service Dependencies

| Dependency       | Version | Purpose |
|------------------|---------|---------|
| PostgreSQL       | 16.x    | Primary data store |
| Redis            | 7.x     | Nonce cache, rate limiting, sessions, provider health |
| Mock LLM Provider| Custom  | Deterministic responses for functional tests |
| Nginx/Envoy      | Latest  | TLS termination in staging |

### 1.2.3 Test Credential Management

- Ed25519 keypairs generated per test run (ephemeral)
- Admin JWT tokens generated with known secrets in test environments
- Provider API keys stored in CI/CD secrets manager (never in code)
- Separate test tenants and service accounts per test suite

---

## 1.3 Test Data Strategy

### 1.3.1 Fixture Data

| Category | Strategy |
|----------|----------|
| Tenants | Pre-seeded set: `test-tenant-active`, `test-tenant-suspended`, `test-tenant-deleted` |
| Service Accounts | Pre-seeded per status: `sa-active`, `sa-suspended`, `sa-revoked` |
| Keys | Generated per test run; includes active, expired, revoked, rotating variants |
| Policies | Fixtures covering: permissive, restrictive, streaming-only, tools-disabled, budget-limited |
| Routes | Fixtures covering: single provider, multi-provider fallback, disabled routes |
| Ed25519 Keypairs | Generated fresh per test suite execution using `ring` or `ed25519-dalek` |

### 1.3.2 Dynamic Data

- Request payloads generated via factory functions with sensible defaults
- Nonces generated as UUID v4 per request to guarantee uniqueness
- Timestamps generated relative to `now()` for window testing

### 1.3.3 Cleanup

- Each test suite operates in a database transaction that rolls back on completion (unit/integration)
- System tests use dedicated tenant namespaces that are truncated after the suite
- Redis keys use test-specific prefixes and are flushed after suite

---

## 1.4 Test Automation Framework Recommendations

### 1.4.1 Backend (Rust)

| Tool | Purpose |
|------|---------|
| `cargo test` | Unit + integration test runner |
| `testcontainers-rs` | Ephemeral PostgreSQL and Redis for integration tests |
| `wiremock` (Rust) | HTTP mock server for provider adapter tests |
| `tokio::test` | Async test runtime |
| `proptest` | Property-based testing for policy evaluation and canonical string construction |
| `criterion` | Micro-benchmarks for signature verification, policy evaluation |

### 1.4.2 Frontend (SvelteKit)

| Tool | Purpose |
|------|---------|
| Vitest | Unit tests for components and stores |
| Playwright | E2E browser tests for admin console |
| Testing Library (Svelte) | Component interaction tests |
| Axe-core | Accessibility auditing |

### 1.4.3 SDK (Python)

| Tool | Purpose |
|------|---------|
| pytest | Test runner |
| pytest-asyncio | Async test support |
| respx / httpx-mock | HTTP mocking for SDK tests |
| hypothesis | Property-based testing for signing logic |

### 1.4.4 Performance

| Tool | Purpose |
|------|---------|
| Grafana k6 (v1.0) | Load testing with TypeScript scripts |
| k6 Operator (v1.0 GA) | Distributed load testing on Kubernetes |
| Prometheus + Grafana | Real-time metrics during load tests |

### 1.4.5 Security

| Tool | Purpose |
|------|---------|
| OWASP ZAP | Automated API security scanning |
| Burp Suite | Manual penetration testing |
| Custom fuzzing scripts | Targeted Ed25519 and header fuzzing |
| `cargo-audit` | Rust dependency vulnerability scanning |
| `npm audit` | Frontend dependency scanning |
| `pip-audit` | Python SDK dependency scanning |

---

## 1.5 Entry/Exit Criteria

### 1.5.1 Unit Testing

| Gate | Criteria |
|------|----------|
| Entry | Code compiles, all dependencies resolved |
| Exit  | >= 80% line coverage on core modules, zero test failures, all P0/P1 cases pass |

### 1.5.2 Integration Testing

| Gate | Criteria |
|------|----------|
| Entry | Unit tests pass, test environment (DB + Redis) available |
| Exit  | All module interaction tests pass, no data integrity issues, all DB migrations apply cleanly |

### 1.5.3 System Testing

| Gate | Criteria |
|------|----------|
| Entry | Integration tests pass, staging environment deployed, mock providers running |
| Exit  | All critical path scenarios pass, no P0/P1 defects open, auth flow fully validated |

### 1.5.4 Performance Testing

| Gate | Criteria |
|------|----------|
| Entry | System tests pass, staging environment at production-equivalent spec |
| Exit  | p95 latency < 200ms overhead at 1000 concurrent, no memory leaks under 1-hour sustained load, streaming TTFT < 100ms overhead |

### 1.5.5 Security Testing

| Gate | Criteria |
|------|----------|
| Entry | System tests pass, security environment deployed |
| Exit  | Zero critical/high vulnerabilities, OWASP API Top 10 checklist complete, replay attack prevention verified, all auth bypass attempts fail |

### 1.5.6 Release

| Gate | Criteria |
|------|----------|
| Entry | All above phases pass |
| Exit  | Release readiness checklist complete, rollback plan documented, monitoring dashboards confirmed |

---

# Part 2: Functional Test Cases

## 2.1 Authentication Module

### 2.1.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AUTH-N-001 | Signature | Valid Ed25519 signature accepted | Active SA, active key, valid keypair | 1. Construct canonical string (METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_SHA256) 2. Sign with private key 3. Send request with all 6 auth headers | 200 OK, request proceeds to policy engine | P0 | Normal |
| AUTH-N-002 | Timestamp | Timestamp within 5-minute window accepted | Active SA, valid key | 1. Set X-Timestamp to current time 2. Sign and send request | Request accepted | P0 | Normal |
| AUTH-N-003 | Timestamp | Timestamp at exactly 5-minute boundary accepted | Active SA, valid key | 1. Set X-Timestamp to exactly now() - 300s 2. Sign and send request | Request accepted (boundary inclusive) | P1 | Normal |
| AUTH-N-004 | Nonce | Unique nonce accepted | Active SA, valid key | 1. Generate UUID v4 nonce 2. Sign and send request | Request accepted, nonce stored in Redis with 300s TTL | P0 | Normal |
| AUTH-N-005 | Key Resolution | Request with active key resolves correctly | SA has multiple keys, one active | 1. Use active key_id in X-Key-Id 2. Sign with corresponding private key | Key resolved, signature verified | P0 | Normal |
| AUTH-N-006 | Key Rotation | Request works during key rotation (rotating status) | Key in `rotating` status | 1. Sign request with rotating key 2. Send request | Request accepted (rotating keys are still valid) | P1 | Normal |
| AUTH-N-007 | Context | Authenticated context populated correctly | Valid request | 1. Send valid signed request 2. Inspect internal context | tenant_id, service_account_id, key_id, policy_id, auth_method all populated | P1 | Normal |
| AUTH-N-008 | Body Hash | Body SHA256 matches request body | Valid request with JSON body | 1. Compute SHA256 of request body 2. Set X-Body-SHA256 3. Include in canonical string 4. Sign and send | Hash verified, request proceeds | P0 | Normal |
| AUTH-N-009 | Empty Body | GET request with empty body hash | GET /v1/models | 1. Compute SHA256 of empty string 2. Sign and send GET request | Request accepted with empty body hash | P1 | Normal |
| AUTH-N-010 | Last Used | Key last_used_at updated on successful auth | Active key | 1. Send valid request 2. Query key record | last_used_at timestamp updated | P2 | Normal |

### 2.1.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AUTH-A-001 | Timestamp | Expired timestamp rejected (older than 5 min) | Active SA | 1. Set X-Timestamp to now() - 301s 2. Sign and send | 401 Unauthorized, error: "timestamp_expired" | P0 | Abnormal |
| AUTH-A-002 | Timestamp | Future timestamp rejected (more than 5 min ahead) | Active SA | 1. Set X-Timestamp to now() + 301s 2. Sign and send | 401 Unauthorized, error: "timestamp_future" | P0 | Abnormal |
| AUTH-A-003 | Nonce | Replayed nonce rejected | Previous request succeeded | 1. Reuse nonce from previous successful request 2. Sign and send | 401 Unauthorized, error: "nonce_replayed" | P0 | Abnormal |
| AUTH-A-004 | Nonce | Replayed nonce from different SA rejected | Nonce used by SA-1 | 1. Use same nonce for SA-2 request | 401 Unauthorized, error: "nonce_replayed" (nonces are global or per-SA depending on design; verify per implementation: nonce:{service_account_id}:{nonce}) | P1 | Abnormal |
| AUTH-A-005 | Signature | Wrong private key produces invalid signature | Valid SA, key-A registered | 1. Sign with key-B's private key 2. Send with key-A's key_id | 401 Unauthorized, error: "signature_invalid" | P0 | Abnormal |
| AUTH-A-006 | Key | Revoked key rejected | Key status = revoked | 1. Sign with revoked key 2. Send request | 401 Unauthorized, error: "key_revoked" | P0 | Abnormal |
| AUTH-A-007 | Key | Expired key rejected | Key expires_at in the past | 1. Sign with expired key 2. Send request | 401 Unauthorized, error: "key_expired" | P0 | Abnormal |
| AUTH-A-008 | Account | Suspended service account rejected | SA status = suspended | 1. Sign with valid key of suspended SA 2. Send request | 401 Unauthorized, error: "service_account_suspended" | P0 | Abnormal |
| AUTH-A-009 | Account | Revoked service account rejected | SA status = revoked | 1. Sign with valid key of revoked SA 2. Send request | 401 Unauthorized, error: "service_account_revoked" | P0 | Abnormal |
| AUTH-A-010 | Headers | Missing X-Service-Account-Id header | Valid request minus one header | 1. Omit X-Service-Account-Id 2. Send request | 401 Unauthorized, error: "missing_header: X-Service-Account-Id" | P0 | Abnormal |
| AUTH-A-011 | Headers | Missing X-Key-Id header | Valid request minus one header | 1. Omit X-Key-Id 2. Send request | 401 Unauthorized, error: "missing_header: X-Key-Id" | P0 | Abnormal |
| AUTH-A-012 | Headers | Missing X-Timestamp header | Valid request minus one header | 1. Omit X-Timestamp 2. Send request | 401 Unauthorized, error: "missing_header: X-Timestamp" | P0 | Abnormal |
| AUTH-A-013 | Headers | Missing X-Nonce header | Valid request minus one header | 1. Omit X-Nonce 2. Send request | 401 Unauthorized, error: "missing_header: X-Nonce" | P0 | Abnormal |
| AUTH-A-014 | Headers | Missing X-Body-SHA256 header | Valid request minus one header | 1. Omit X-Body-SHA256 2. Send request | 401 Unauthorized, error: "missing_header: X-Body-SHA256" | P0 | Abnormal |
| AUTH-A-015 | Headers | Missing X-Signature header | Valid request minus one header | 1. Omit X-Signature 2. Send request | 401 Unauthorized, error: "missing_header: X-Signature" | P0 | Abnormal |
| AUTH-A-016 | Headers | All auth headers missing | No auth headers | 1. Send request with no auth headers | 401 Unauthorized, error: "missing_authentication" | P0 | Abnormal |
| AUTH-A-017 | Body Tampering | Body modified after signing | Valid signed request | 1. Sign request with original body 2. Modify body JSON after signing 3. Send | 401 Unauthorized, error: "body_hash_mismatch" | P0 | Abnormal |
| AUTH-A-018 | Body Tampering | Body SHA256 header does not match body | Mismatched hash | 1. Set X-Body-SHA256 to hash of different body 2. Send | 401 Unauthorized, error: "body_hash_mismatch" | P0 | Abnormal |
| AUTH-A-019 | Signature Format | Invalid base64 in X-Signature | Malformed signature | 1. Set X-Signature to "not-valid-base64!!!" 2. Send | 401 Unauthorized, error: "signature_format_invalid" | P1 | Abnormal |
| AUTH-A-020 | Signature Format | Signature of wrong length | Truncated signature | 1. Truncate valid signature to half length 2. Send | 401 Unauthorized, error: "signature_format_invalid" | P1 | Abnormal |
| AUTH-A-021 | Algorithm | Non-Ed25519 key rejected | Key registered with algorithm != ed25519 | 1. Attempt to use RSA-signed request 2. Send | 401 Unauthorized, error: "unsupported_algorithm" | P1 | Abnormal |
| AUTH-A-022 | Key | Non-existent key_id | key_id not in database | 1. Set X-Key-Id to random UUID 2. Send | 401 Unauthorized, error: "key_not_found" | P0 | Abnormal |
| AUTH-A-023 | Account | Non-existent service_account_id | SA ID not in database | 1. Set X-Service-Account-Id to random UUID 2. Send | 401 Unauthorized, error: "service_account_not_found" | P0 | Abnormal |
| AUTH-A-024 | Timestamp | Invalid timestamp format | Non-ISO-8601 timestamp | 1. Set X-Timestamp to "not-a-timestamp" 2. Send | 401 Unauthorized, error: "timestamp_format_invalid" | P1 | Abnormal |
| AUTH-A-025 | Nonce | Empty nonce string | X-Nonce = "" | 1. Set X-Nonce to empty string 2. Send | 401 Unauthorized, error: "nonce_empty" | P1 | Abnormal |
| AUTH-A-026 | Canonical String | Path traversal in request path | Request to /../admin/... | 1. Send request with path traversal in URL 2. Verify canonical string uses normalized path | 400 Bad Request or 404 Not Found, path normalized before canonical string | P1 | Abnormal |
| AUTH-A-027 | Tenant | Suspended tenant blocks all SAs | Tenant status = suspended | 1. Sign with valid SA key under suspended tenant 2. Send | 401 or 403, error: "tenant_suspended" | P0 | Abnormal |
| AUTH-A-028 | Concurrency | Same nonce sent concurrently from two requests | Race condition test | 1. Send two requests with identical nonce simultaneously | Exactly one succeeds, the other gets 401 nonce_replayed | P1 | Abnormal |

---

## 2.2 API Layer -- Chat Completions

### 2.2.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| CHAT-N-001 | Basic | Simple chat completion with single user message | Authenticated, model allowed | 1. POST /v1/chat/completions with model + messages=[{role:user, content:"Hello"}] | 200 OK, response contains id, object, created, model, choices, usage | P0 | Normal |
| CHAT-N-002 | System | Chat with system message | Authenticated | 1. Send messages=[{role:system, content:"You are helpful"}, {role:user, content:"Hi"}] | 200 OK, system prompt applied | P0 | Normal |
| CHAT-N-003 | Multi-turn | Multi-turn conversation | Authenticated | 1. Send messages=[system, user, assistant, user] | 200 OK, response considers conversation history | P1 | Normal |
| CHAT-N-004 | Temperature | Chat with temperature parameter | Authenticated | 1. Set temperature=0.7 2. Send request | 200 OK, temperature forwarded to provider | P1 | Normal |
| CHAT-N-005 | Top-p | Chat with top_p parameter | Authenticated | 1. Set top_p=0.9 2. Send request | 200 OK, top_p forwarded to provider | P2 | Normal |
| CHAT-N-006 | Max Tokens | Chat with max_tokens limit | Authenticated | 1. Set max_tokens=100 2. Send request | 200 OK, response.usage.completion_tokens <= 100 | P1 | Normal |
| CHAT-N-007 | Usage | Usage fields populated in response | Authenticated | 1. Send chat request | 200 OK, usage.prompt_tokens > 0, usage.completion_tokens > 0, usage.total_tokens = prompt + completion | P0 | Normal |
| CHAT-N-008 | Request ID | Response contains unique request_id | Authenticated | 1. Send chat request 2. Check response headers/body | Response includes unique request_id for tracing | P1 | Normal |
| CHAT-N-009 | Model Echo | Response echoes resolved model name | Authenticated | 1. Request with model="gpt-4" (alias) | 200 OK, response.model reflects the actual provider model name | P1 | Normal |
| CHAT-N-010 | Content Array | Multi-part content (text + image_url) | Authenticated, model supports vision | 1. Send content as array [{type:"text", text:"..."}, {type:"image_url", image_url:{url:"..."}}] | 200 OK, provider receives transformed multi-part content | P2 | Normal |

### 2.2.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| CHAT-A-001 | Validation | Missing model field | Authenticated | 1. POST /v1/chat/completions without "model" field | 400 Bad Request, error: "missing_field: model" | P0 | Abnormal |
| CHAT-A-002 | Validation | Missing messages field | Authenticated | 1. POST without "messages" field | 400 Bad Request, error: "missing_field: messages" | P0 | Abnormal |
| CHAT-A-003 | Validation | Empty messages array | Authenticated | 1. Send messages=[] | 400 Bad Request, error: "messages_empty" | P0 | Abnormal |
| CHAT-A-004 | Validation | Invalid role in message | Authenticated | 1. Send messages=[{role:"invalid_role", content:"Hi"}] | 400 Bad Request, error: "invalid_role" | P1 | Abnormal |
| CHAT-A-005 | Validation | Message missing content field | Authenticated | 1. Send messages=[{role:"user"}] | 400 Bad Request, error: "missing_field: content" | P1 | Abnormal |
| CHAT-A-006 | Payload | Oversized request payload (>10MB) | Authenticated | 1. Construct messages with > 10MB total content 2. Send request | 413 Payload Too Large or 400 with size error | P1 | Abnormal |
| CHAT-A-007 | Payload | Invalid JSON body | Authenticated | 1. Send body = "{invalid json" | 400 Bad Request, error: "invalid_json" | P0 | Abnormal |
| CHAT-A-008 | Payload | Empty request body | Authenticated | 1. Send POST with empty body | 400 Bad Request, error: "empty_body" | P1 | Abnormal |
| CHAT-A-009 | Model | Unsupported/unknown model | Authenticated | 1. Set model="nonexistent-model-xyz" | 404 or 400, error: "model_not_found" | P0 | Abnormal |
| CHAT-A-010 | Parameters | Temperature out of range (>2.0) | Authenticated | 1. Set temperature=5.0 | 400 Bad Request, error: "temperature_out_of_range" | P2 | Abnormal |
| CHAT-A-011 | Parameters | Negative max_tokens | Authenticated | 1. Set max_tokens=-1 | 400 Bad Request, error: "invalid_max_tokens" | P2 | Abnormal |
| CHAT-A-012 | Parameters | top_p out of range | Authenticated | 1. Set top_p=1.5 | 400 Bad Request, error: "top_p_out_of_range" | P2 | Abnormal |
| CHAT-A-013 | Content-Type | Wrong Content-Type header | Authenticated | 1. Set Content-Type: text/plain 2. Send request | 415 Unsupported Media Type | P1 | Abnormal |
| CHAT-A-014 | Method | GET instead of POST | Authenticated | 1. GET /v1/chat/completions | 405 Method Not Allowed | P1 | Abnormal |
| CHAT-A-015 | Provider Error | Provider returns 500 and no fallback available | Single route, provider down | 1. Send valid request 2. Provider returns 500 | 502 Bad Gateway or 500, error includes provider context | P1 | Abnormal |
| CHAT-A-016 | Provider Error | Provider returns 429 rate limit | Provider rate limited | 1. Send valid request 2. Provider returns 429 | Gateway retries per config, eventually returns 429 with appropriate message | P1 | Abnormal |
| CHAT-A-017 | Timeout | Provider does not respond within timeout_ms | Route timeout = 5000ms | 1. Send request 2. Provider delays > 5s | 504 Gateway Timeout | P1 | Abnormal |
| CHAT-A-018 | Unicode | Malformed UTF-8 in message content | Authenticated | 1. Send message with invalid UTF-8 bytes | 400 Bad Request | P2 | Abnormal |
| CHAT-A-019 | Injection | SQL injection in model field | Authenticated | 1. Set model="'; DROP TABLE tenants;--" | 400 or safe handling, no SQL executed | P0 | Abnormal |
| CHAT-A-020 | Injection | JSON injection in messages | Authenticated | 1. Attempt to inject extra fields via nested JSON | Ignored fields stripped or rejected, no impact | P1 | Abnormal |

---

## 2.3 API Layer -- Streaming

### 2.3.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| STRM-N-001 | Basic | stream=true returns SSE response | Authenticated, streaming allowed | 1. POST /v1/chat/completions with stream=true | 200 OK, Content-Type: text/event-stream, SSE chunks received | P0 | Normal |
| STRM-N-002 | Events | All event types received in order | Authenticated | 1. Send streaming request 2. Collect all events | Events received: response.started -> response.output_text.delta (1+) -> response.completed | P0 | Normal |
| STRM-N-003 | Completion | Stream completes with [DONE] marker | Authenticated | 1. Send streaming request 2. Wait for completion | Final event is data: [DONE] | P0 | Normal |
| STRM-N-004 | Usage | Usage reported in final event | Authenticated | 1. Send streaming request 2. Check final event | Final completion event includes usage (prompt_tokens, completion_tokens) | P1 | Normal |
| STRM-N-005 | Content | Delta content concatenation matches non-stream | Authenticated | 1. Send same prompt with stream=true and stream=false 2. Compare | Concatenated deltas produce equivalent content to non-streaming response | P1 | Normal |
| STRM-N-006 | Tool Calls | Streaming with tool_call deltas | Authenticated, tools allowed | 1. Send request that triggers tool use with stream=true | response.tool_call.delta events received with function name and arguments | P2 | Normal |
| STRM-N-007 | Multi-model | Streaming works across all providers | Authenticated, routes to OpenAI + Anthropic | 1. Send streaming request to each provider | SSE works correctly regardless of upstream provider format | P1 | Normal |
| STRM-N-008 | Responses API | POST /v1/responses with stream=true | Authenticated | 1. Send to /v1/responses with stream=true | Streaming response with normalized event format | P1 | Normal |

### 2.3.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| STRM-A-001 | Disconnect | Client disconnects mid-stream | Authenticated | 1. Start streaming request 2. Close client connection after receiving 3 chunks | Gateway detects disconnect, cancels upstream provider request, usage recorded for partial response | P0 | Abnormal |
| STRM-A-002 | Timeout | Upstream provider times out during stream | Authenticated | 1. Start streaming 2. Provider stops sending after 2 chunks, exceeds timeout | Gateway sends error event to client, closes stream, records partial usage | P1 | Abnormal |
| STRM-A-003 | Malformed | Upstream sends malformed SSE data | Authenticated | 1. Provider sends invalid SSE format | Gateway handles gracefully, sends error event, closes stream | P1 | Abnormal |
| STRM-A-004 | Permission | stream=true but streaming disabled by policy | Policy: allow_streaming=false | 1. Send request with stream=true | 403 Forbidden, error: "streaming_not_allowed" | P0 | Abnormal |
| STRM-A-005 | Provider Down | Provider connection drops during stream | Authenticated | 1. Start streaming 2. Provider TCP connection resets | Gateway sends error event, attempts fallback if configured | P1 | Abnormal |
| STRM-A-006 | Empty Stream | Provider returns empty stream (started + completed, no deltas) | Authenticated | 1. Send request that triggers empty response | Client receives started + completed events, usage shows 0 completion tokens | P2 | Abnormal |
| STRM-A-007 | Concurrent | Many concurrent streams from same SA | Authenticated | 1. Open 50 concurrent streaming requests | All streams maintained independently, no cross-contamination | P1 | Abnormal |
| STRM-A-008 | Budget | Budget exhausted mid-stream | Budget nearly exhausted | 1. Start streaming 2. Budget crosses threshold mid-stream | Current stream completes (already in-flight), subsequent requests denied | P1 | Abnormal |

---

## 2.4 API Layer -- Embeddings

### 2.4.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| EMB-N-001 | Basic | Single text embedding | Authenticated, embedding model configured | 1. POST /v1/embeddings with model + input="Hello world" | 200 OK, response contains object:"list", data array with embedding vector | P0 | Normal |
| EMB-N-002 | Batch | Batch text embeddings | Authenticated | 1. Send input=["text1", "text2", "text3"] | 200 OK, data array has 3 items, each with index and embedding | P0 | Normal |
| EMB-N-003 | Model | Valid embedding model resolves | Authenticated | 1. Set model="text-embedding-3-small" (alias) | 200 OK, model routed to correct provider | P1 | Normal |
| EMB-N-004 | Usage | Token usage reported | Authenticated | 1. Send embedding request | 200 OK, usage.prompt_tokens > 0, usage.total_tokens > 0 | P1 | Normal |
| EMB-N-005 | Dimensions | Embedding dimensions match model spec | Authenticated | 1. Send embedding request 2. Check vector length | Each embedding vector has expected dimension count | P2 | Normal |

### 2.4.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| EMB-A-001 | Validation | Empty input string | Authenticated | 1. Send input="" | 400 Bad Request, error: "input_empty" | P0 | Abnormal |
| EMB-A-002 | Validation | Empty input array | Authenticated | 1. Send input=[] | 400 Bad Request, error: "input_empty" | P0 | Abnormal |
| EMB-A-003 | Batch Limit | Too many inputs in batch | Authenticated | 1. Send input array with > max allowed items (e.g., 2048+) | 400 Bad Request, error: "batch_size_exceeded" | P1 | Abnormal |
| EMB-A-004 | Model | Unsupported model for embeddings | Authenticated | 1. Set model to a chat model, not an embedding model | 400 Bad Request, error: "model_not_supported_for_embeddings" | P1 | Abnormal |
| EMB-A-005 | Model | Unknown model | Authenticated | 1. Set model="nonexistent-embedding-model" | 404 or 400, error: "model_not_found" | P0 | Abnormal |
| EMB-A-006 | Input | Input text exceeds model token limit | Authenticated | 1. Send single input with > max tokens for the model | 400 Bad Request, error: "input_too_long" | P1 | Abnormal |
| EMB-A-007 | Validation | Missing model field | Authenticated | 1. Omit model from request | 400 Bad Request, error: "missing_field: model" | P0 | Abnormal |
| EMB-A-008 | Validation | Missing input field | Authenticated | 1. Omit input from request | 400 Bad Request, error: "missing_field: input" | P0 | Abnormal |
| EMB-A-009 | Type | Input is neither string nor array | Authenticated | 1. Send input=12345 (integer) | 400 Bad Request, error: "invalid_input_type" | P2 | Abnormal |

---

## 2.5 Policy Engine

### 2.5.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| POL-N-001 | Model Access | Request for allowed model passes | Policy allows "gpt-4" | 1. Send request with model="gpt-4" | Request proceeds past policy check | P0 | Normal |
| POL-N-002 | Model Access | Wildcard model pattern matches | Policy allows "gpt-*" | 1. Send request with model="gpt-4-turbo" | Request allowed by wildcard match | P1 | Normal |
| POL-N-003 | Token Limits | Request within input token limit | Policy max_input_tokens=4096, request has 1000 tokens | 1. Send request with small prompt | Policy check passes | P0 | Normal |
| POL-N-004 | Token Limits | Request within output token limit | Policy max_output_tokens=2048, request max_tokens=1000 | 1. Send request with max_tokens=1000 | Policy check passes | P0 | Normal |
| POL-N-005 | Features | Streaming request with streaming enabled | Policy allow_streaming=true | 1. Send request with stream=true | Policy check passes | P0 | Normal |
| POL-N-006 | Features | Tool use request with tools enabled | Policy allow_tools=true | 1. Send request with tools parameter | Policy check passes | P1 | Normal |
| POL-N-007 | Features | File input request with files enabled | Policy allow_files=true | 1. Send request with file content | Policy check passes | P1 | Normal |
| POL-N-008 | Budget | Request within daily budget | Daily budget=$100, spent=$50 | 1. Send request | Policy check passes, budget not exhausted | P0 | Normal |
| POL-N-009 | Budget | Request within monthly budget | Monthly budget=$1000, spent=$500 | 1. Send request | Policy check passes | P0 | Normal |
| POL-N-010 | RPM | Request within RPM limit | RPM limit=60, current count=30 | 1. Send request | Policy check passes | P0 | Normal |
| POL-N-011 | Multiple Policies | SA with multiple policy bindings, most permissive applies | SA bound to policy-A (restrictive) and policy-B (permissive) | 1. Send request allowed by policy-B but denied by policy-A | Verify which merge strategy applies (union vs intersection) -- document actual behavior | P1 | Normal |

### 2.5.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| POL-A-001 | Model Access | Request for denied model | Policy denied_models=["gpt-4"] | 1. Send request with model="gpt-4" | 403 Forbidden, error: "model_denied" | P0 | Abnormal |
| POL-A-002 | Model Access | Model not in allowed list | Policy allowed_models=["gpt-3.5-turbo"] | 1. Send request with model="gpt-4" | 403 Forbidden, error: "model_not_allowed" | P0 | Abnormal |
| POL-A-003 | Model Access | Deny list takes precedence over allow list | Policy allows "gpt-*", denies "gpt-4" | 1. Send request with model="gpt-4" | 403 Forbidden, error: "model_denied" (deny overrides allow) | P0 | Abnormal |
| POL-A-004 | Token Limits | Input tokens exceed max_input_tokens | Policy max_input_tokens=1000, request has 2000 tokens | 1. Send request with long prompt | 403 Forbidden, error: "input_tokens_exceeded" | P0 | Abnormal |
| POL-A-005 | Token Limits | Requested output tokens exceed max_output_tokens | Policy max_output_tokens=500, request max_tokens=1000 | 1. Send request with max_tokens=1000 | 403 Forbidden, error: "output_tokens_exceeded" or max_tokens clamped to 500 | P0 | Abnormal |
| POL-A-006 | Features | Streaming disabled | Policy allow_streaming=false | 1. Send request with stream=true | 403 Forbidden, error: "streaming_not_allowed" | P0 | Abnormal |
| POL-A-007 | Features | Tools disabled | Policy allow_tools=false | 1. Send request with tools parameter | 403 Forbidden, error: "tools_not_allowed" | P1 | Abnormal |
| POL-A-008 | Features | Files disabled | Policy allow_files=false | 1. Send request with file content | 403 Forbidden, error: "files_not_allowed" | P1 | Abnormal |
| POL-A-009 | Budget | Daily budget exhausted | Daily budget=$10, spent=$10.01 | 1. Send request | 403 Forbidden, error: "daily_budget_exhausted" | P0 | Abnormal |
| POL-A-010 | Budget | Monthly budget exhausted | Monthly budget=$100, spent=$100.01 | 1. Send request | 403 Forbidden, error: "monthly_budget_exhausted" | P0 | Abnormal |
| POL-A-011 | Region | Region restriction violated | Policy allowed_regions=["us-east"], request would route to eu-west | 1. Send request | 403 Forbidden, error: "region_not_allowed" | P1 | Abnormal |
| POL-A-012 | No Policy | SA with no policy binding | SA has no default_policy_id and no bindings | 1. Send request | 403 Forbidden, error: "no_policy_configured" or default-deny behavior | P1 | Abnormal |
| POL-A-013 | Concurrency | Concurrency limit exceeded | Policy concurrency_limit=5, 5 in-flight requests | 1. Send 6th concurrent request | 429 Too Many Requests, error: "concurrency_limit_exceeded" | P1 | Abnormal |

---

## 2.6 Routing Engine

### 2.6.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ROUTE-N-001 | Resolution | Model alias resolves to primary route | model_alias="gpt-4" mapped to OpenAI, priority=1 | 1. Send request with model="gpt-4" | Request routed to OpenAI provider with provider_model_name | P0 | Normal |
| ROUTE-N-002 | Priority | Highest priority route selected first | Two routes: OpenAI priority=1, Anthropic priority=2 | 1. Send request | Route with priority=1 (lowest number) selected | P0 | Normal |
| ROUTE-N-003 | Fallback | Fallback triggered on primary failure | Primary route fails (500), fallback route exists | 1. Send request 2. Primary provider returns 500 | Request retried on fallback provider, 200 OK | P0 | Normal |
| ROUTE-N-004 | Retry | Retry succeeds on transient error | Route max_retries=2, provider fails once then succeeds | 1. Send request 2. First attempt fails with 503 3. Second attempt succeeds | 200 OK, retry_count=1 in usage event | P0 | Normal |
| ROUTE-N-005 | Backoff | Exponential backoff between retries | Route retry_backoff_ms=250, max_retries=3 | 1. Force provider failures 2. Measure retry timing | Delays: ~250ms, ~500ms, ~1000ms (exponential) | P1 | Normal |
| ROUTE-N-006 | Tenant Routes | Tenant-specific route overrides global | Global route priority=10, tenant route priority=1 | 1. Send request as tenant with override | Tenant-specific route selected | P1 | Normal |
| ROUTE-N-007 | Disabled Skip | Disabled route skipped | Route A enabled=true priority=1, Route B enabled=false priority=0 | 1. Send request | Route A selected (B skipped despite lower priority number) | P1 | Normal |
| ROUTE-N-008 | Timeout Config | Route-specific timeout applied | Route timeout_ms=5000 | 1. Send request 2. Provider responds in 4s | 200 OK, within timeout | P1 | Normal |

### 2.6.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ROUTE-A-001 | All Down | All providers in fallback chain fail | 3 routes, all return 500 | 1. Send request | 502 Bad Gateway, error includes attempted providers and failure reasons | P0 | Abnormal |
| ROUTE-A-002 | No Routes | No routes configured for model alias | model_alias="unconfigured-model" | 1. Send request with model="unconfigured-model" | 404 or 400, error: "no_routes_for_model" | P0 | Abnormal |
| ROUTE-A-003 | All Disabled | All routes for model are disabled | Routes exist but all enabled=false | 1. Send request | 404 or 503, error: "no_active_routes" | P1 | Abnormal |
| ROUTE-A-004 | Timeout All | Timeout on all retry attempts | Route max_retries=2, provider always times out | 1. Send request | 504 Gateway Timeout after all retries exhausted | P1 | Abnormal |
| ROUTE-A-005 | Circular | Circular fallback (same provider retried) | Route A falls back to Route A (config error) | 1. Send request, provider fails | System detects circular fallback, does not loop infinitely | P1 | Abnormal |
| ROUTE-A-006 | Auth Error | Provider authentication error (invalid API key) | Provider credentials misconfigured | 1. Send request | 502 or appropriate error, error: "provider_auth_error", not exposing provider credentials | P0 | Abnormal |
| ROUTE-A-007 | Partial | Fallback provider returns different model | Primary: gpt-4, Fallback: claude-3-sonnet | 1. Primary fails 2. Fallback succeeds | 200 OK, response.model reflects actual model used, usage event tracks actual provider | P1 | Abnormal |
| ROUTE-A-008 | Rate Limited | Provider rate-limits during retry chain | Provider returns 429 on retry | 1. Send request 2. Provider returns 429 | Try next fallback or return 429 if all exhausted | P1 | Abnormal |

---

## 2.7 Rate Limiting

### 2.7.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| RLIM-N-001 | Within Limit | Request within SA RPM limit | SA RPM limit=60, current=0 | 1. Send request | Request accepted, rate counter incremented | P0 | Normal |
| RLIM-N-002 | Approaching | Request at 90% of limit | SA RPM limit=100, current=90 | 1. Send request | Request accepted, rate limit headers show remaining=9 | P1 | Normal |
| RLIM-N-003 | Window Reset | Rate limit window resets after sliding window expires | SA limit hit, wait for window to slide | 1. Hit limit 2. Wait for window to slide past oldest request 3. Send new request | Request accepted after window slides | P1 | Normal |
| RLIM-N-004 | Headers | Rate limit headers present in response | Any authenticated request | 1. Send request | Response includes X-RateLimit-Limit, X-RateLimit-Remaining, X-RateLimit-Reset headers | P1 | Normal |
| RLIM-N-005 | Multi-Level | Tenant limit and SA limit both checked | Tenant RPM=1000, SA RPM=100 | 1. Send request within both limits | Both counters incremented, request accepted | P0 | Normal |

### 2.7.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| RLIM-A-001 | SA Limit | Exceeded per-SA RPM limit | SA RPM limit=10, 10 requests already made in window | 1. Send 11th request | 429 Too Many Requests, Retry-After header present | P0 | Abnormal |
| RLIM-A-002 | Tenant Limit | Exceeded per-tenant RPM limit | Tenant RPM limit=100, 100 requests from all SAs | 1. Send 101st request | 429 Too Many Requests at tenant level | P0 | Abnormal |
| RLIM-A-003 | Global Limit | Exceeded global RPM limit | Global limit hit | 1. Send request beyond global limit | 429 Too Many Requests | P1 | Abnormal |
| RLIM-A-004 | Burst | Burst of requests at exactly the limit | SA RPM=60 | 1. Send 60 requests in 1 second | All 60 accepted (within RPM for the minute window), 61st rejected | P1 | Abnormal |
| RLIM-A-005 | Concurrent Boundary | Concurrent requests at exact boundary | SA RPM=10, 9 requests made | 1. Send 2 requests concurrently (both see count=9) | Exactly one succeeds, one gets 429 (atomic increment) | P0 | Abnormal |
| RLIM-A-006 | Redis Down | Rate limiting when Redis is unavailable | Redis connection lost | 1. Send request | Fail-open or fail-closed per configuration (document actual behavior) | P1 | Abnormal |
| RLIM-A-007 | SA + Tenant | SA within limit but tenant exceeded | SA RPM=10 (3 used), Tenant RPM=100 (100 used) | 1. Send request | 429 at tenant level despite SA having capacity | P1 | Abnormal |
| RLIM-A-008 | No Limit | SA with no RPM limit configured (null) | Policy rpm_limit=null | 1. Send many requests quickly | No rate limiting applied (or default limit applied -- verify behavior) | P2 | Abnormal |

---

## 2.8 Usage & Budget

### 2.8.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| USAGE-N-001 | Recording | Usage event recorded for successful request | Authenticated, valid request | 1. Send chat request 2. Query usage_events table | Row exists with correct request_id, tenant_id, SA_id, tokens, cost, latency, status | P0 | Normal |
| USAGE-N-002 | Tokens | Token counts match provider response | Authenticated | 1. Send request 2. Compare usage_events.prompt_tokens and completion_tokens with provider response | Token counts match | P0 | Normal |
| USAGE-N-003 | Cost | Cost estimated correctly | Model pricing configured | 1. Send request with known token count 2. Check estimated_cost | Cost = (prompt_tokens * input_price + completion_tokens * output_price) | P0 | Normal |
| USAGE-N-004 | Budget Track | Budget accumulation correct | Daily budget configured | 1. Send multiple requests 2. Sum estimated_cost | Running total matches sum of individual costs | P0 | Normal |
| USAGE-N-005 | Streaming | Usage recorded for streaming request | Authenticated, streaming | 1. Send streaming request 2. Let it complete 3. Check usage_events | is_streaming=true, tokens and cost recorded correctly | P1 | Normal |
| USAGE-N-006 | Retry | Usage recorded with retry_count | Request required retries | 1. Send request that retries once 2. Check usage_events | retry_count=1, final provider tracked | P1 | Normal |
| USAGE-N-007 | Latency | Latency_ms recorded accurately | Authenticated | 1. Send request 2. Check latency_ms | Latency reflects actual end-to-end time within reasonable margin | P1 | Normal |
| USAGE-N-008 | Aggregation | Usage aggregation by tenant correct | Multiple SAs under tenant | 1. Send requests from SA-1 and SA-2 2. Query tenant aggregate | Sum of all SA usage equals tenant total | P1 | Normal |

### 2.8.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| USAGE-A-001 | Budget Mid-Request | Budget exhausted mid-request (non-streaming) | Budget nearly exhausted, request cost exceeds remaining | 1. Send request that will push over budget | Request completes (already in-flight) but budget updated; next request denied | P0 | Abnormal |
| USAGE-A-002 | Budget Mid-Stream | Budget exceeded during streaming response | Budget nearly exhausted | 1. Start streaming request 2. Cost exceeds budget mid-stream | Current stream completes, budget marked exhausted, next request denied | P1 | Abnormal |
| USAGE-A-003 | Zero Cost | Zero-cost model usage | Model with $0 pricing | 1. Send request to zero-cost model 2. Check usage event | estimated_cost=0, tokens still tracked, budget not affected | P1 | Abnormal |
| USAGE-A-004 | Race Condition | Concurrent budget checks (race condition) | Budget nearly exhausted, two concurrent requests | 1. Send two requests simultaneously, each would push budget over | At most one should succeed and push over budget; verify no negative budget or double-spend | P0 | Abnormal |
| USAGE-A-005 | Failed Request | Usage recorded for failed request | Provider error | 1. Send request, provider returns error | Usage event recorded with final_status="error", cost may be 0 or partial | P1 | Abnormal |
| USAGE-A-006 | Partial Stream | Usage for partial stream (client disconnect) | Client disconnects mid-stream | 1. Start stream 2. Disconnect 3. Check usage_events | Usage recorded with actual tokens delivered, final_status="partial" | P1 | Abnormal |
| USAGE-A-007 | Provider Mismatch | Provider reports different tokens than expected | Provider returns unusual token counts | 1. Send request 2. Provider reports negative prompt_tokens | Gateway handles gracefully, records 0 or flags anomaly | P2 | Abnormal |
| USAGE-A-008 | DB Failure | Usage persistence fails (DB error) | Database temporarily unavailable | 1. Send request 2. Usage INSERT fails | Request still succeeds for the client, usage queued for retry or logged for reconciliation | P1 | Abnormal |

---

## 2.9 Admin API -- Tenants

### 2.9.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ATNT-N-001 | Create | Create tenant with valid data | Admin authenticated | 1. POST /admin/tenants with name="Test Corp", slug="test-corp" | 201 Created, tenant returned with id, status="active" | P0 | Normal |
| ATNT-N-002 | List | List all tenants | Tenants exist | 1. GET /admin/tenants | 200 OK, array of tenants returned | P0 | Normal |
| ATNT-N-003 | Get | Get tenant by ID | Tenant exists | 1. GET /admin/tenants/{id} | 200 OK, tenant details returned | P0 | Normal |
| ATNT-N-004 | Update Status | Update tenant status to suspended | Tenant status=active | 1. PATCH /admin/tenants/{id} with status="suspended" | 200 OK, status updated, audit event logged | P0 | Normal |
| ATNT-N-005 | Update Status | Reactivate suspended tenant | Tenant status=suspended | 1. PATCH with status="active" | 200 OK, tenant reactivated | P1 | Normal |
| ATNT-N-006 | Filter | List tenants with status filter | Mixed status tenants | 1. GET /admin/tenants?status=active | 200 OK, only active tenants returned | P1 | Normal |
| ATNT-N-007 | Pagination | List tenants with pagination | Many tenants | 1. GET /admin/tenants?limit=10&offset=0 | 200 OK, paginated result with metadata | P2 | Normal |

### 2.9.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ATNT-A-001 | Duplicate | Create tenant with duplicate slug | Tenant "test-corp" exists | 1. POST with slug="test-corp" | 409 Conflict, error: "slug_already_exists" | P0 | Abnormal |
| ATNT-A-002 | Validation | Create tenant with empty name | Admin authenticated | 1. POST with name="" | 400 Bad Request, error: "name_required" | P1 | Abnormal |
| ATNT-A-003 | Validation | Create tenant with empty slug | Admin authenticated | 1. POST with slug="" | 400 Bad Request, error: "slug_required" | P1 | Abnormal |
| ATNT-A-004 | Validation | Slug with invalid characters | Admin authenticated | 1. POST with slug="Test Corp!!" | 400 Bad Request, error: "slug_invalid_format" | P1 | Abnormal |
| ATNT-A-005 | Transition | Invalid status transition (active -> deleted directly if not allowed) | Tenant status=active | 1. PATCH with status="deleted" | 400 or 409, error: "invalid_status_transition" (if policy requires active->suspended->deleted) | P1 | Abnormal |
| ATNT-A-006 | Delete | Delete tenant with active service accounts | Tenant has active SAs | 1. Attempt to delete/suspend tenant | Verify cascade behavior: SAs are suspended/blocked, or operation prevented with error | P0 | Abnormal |
| ATNT-A-007 | Not Found | Get non-existent tenant | ID does not exist | 1. GET /admin/tenants/{random-uuid} | 404 Not Found | P1 | Abnormal |
| ATNT-A-008 | Auth | Admin endpoint without JWT | No auth header | 1. Call admin endpoint without Authorization header | 401 Unauthorized | P0 | Abnormal |
| ATNT-A-009 | Auth | Admin endpoint with expired JWT | Expired token | 1. Call with expired JWT | 401 Unauthorized, error: "token_expired" | P0 | Abnormal |

---

## 2.10 Admin API -- Service Accounts

### 2.10.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ASA-N-001 | Create | Create service account under active tenant | Active tenant exists | 1. POST /admin/tenants/{tid}/service-accounts with name, slug, environment | 201 Created, SA with status="active" | P0 | Normal |
| ASA-N-002 | List | List service accounts for tenant | SAs exist | 1. GET /admin/tenants/{tid}/service-accounts | 200 OK, array of SAs | P0 | Normal |
| ASA-N-003 | Get | Get service account by ID | SA exists | 1. GET /admin/service-accounts/{id} | 200 OK, SA details with tenant info | P0 | Normal |
| ASA-N-004 | Update | Update SA status to suspended | SA status=active | 1. PATCH with status="suspended" | 200 OK, status updated, audit event logged | P0 | Normal |
| ASA-N-005 | Environment | Create SA with environment="production" | Active tenant | 1. POST with environment="production" | 201 Created with environment field set | P1 | Normal |
| ASA-N-006 | Policy | Create SA with default_policy_id | Policy exists in tenant | 1. POST with default_policy_id=policy-uuid | 201 Created with policy binding | P1 | Normal |

### 2.10.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| ASA-A-001 | Suspended Tenant | Create SA under suspended tenant | Tenant status=suspended | 1. POST to create SA | 400 or 403, error: "tenant_suspended" | P0 | Abnormal |
| ASA-A-002 | Duplicate | Duplicate slug within same tenant | SA "my-service" exists in tenant | 1. POST with slug="my-service" under same tenant | 409 Conflict, error: "slug_already_exists_in_tenant" | P0 | Abnormal |
| ASA-A-003 | Duplicate | Same slug in different tenants allowed | SA "my-service" in tenant-A | 1. POST with slug="my-service" under tenant-B | 201 Created (UNIQUE constraint is (tenant_id, slug)) | P1 | Abnormal |
| ASA-A-004 | Environment | Invalid environment value | Active tenant | 1. POST with environment="invalid-env" | 400 Bad Request, error: "invalid_environment" | P1 | Abnormal |
| ASA-A-005 | Policy | Default policy from different tenant | Policy belongs to tenant-B | 1. Create SA in tenant-A with default_policy_id from tenant-B | 400 or 403, error: "policy_not_in_tenant" | P1 | Abnormal |
| ASA-A-006 | Not Found | Get SA under wrong tenant | SA belongs to tenant-A | 1. GET /admin/tenants/{tenant-B}/service-accounts/{sa-from-A} | 404 Not Found (tenant scoping enforced) | P1 | Abnormal |
| ASA-A-007 | Validation | Empty slug | Active tenant | 1. POST with slug="" | 400 Bad Request | P1 | Abnormal |
| ASA-A-008 | Validation | Empty name | Active tenant | 1. POST with name="" | 400 Bad Request | P1 | Abnormal |

---

## 2.11 Admin API -- Keys

### 2.11.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AKEY-N-001 | Register | Register Ed25519 public key | Active SA | 1. POST /admin/service-accounts/{said}/keys with key_id, algorithm="ed25519", public_key_pem | 201 Created, key with status="active", fingerprint generated | P0 | Normal |
| AKEY-N-002 | List | List keys for service account | Keys exist | 1. GET /admin/service-accounts/{said}/keys | 200 OK, array of keys with status | P0 | Normal |
| AKEY-N-003 | Revoke | Revoke an active key | Key status=active | 1. POST /admin/service-accounts/{said}/keys/{kid}/revoke | 200 OK, status="revoked", revoked_at set, audit event logged | P0 | Normal |
| AKEY-N-004 | Multiple Keys | Register second key for rotation | SA already has one active key | 1. Register second key | 201 Created, both keys active (supports rotation) | P1 | Normal |
| AKEY-N-005 | Expiry | Register key with expires_at | Active SA | 1. POST with expires_at in the future | 201 Created with expires_at set | P1 | Normal |

### 2.11.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AKEY-A-001 | Duplicate | Register key with duplicate key_id | key_id already exists globally | 1. POST with existing key_id | 409 Conflict, error: "key_id_already_exists" | P0 | Abnormal |
| AKEY-A-002 | Revoke Revoked | Revoke already-revoked key | Key status=revoked | 1. POST revoke | 400 or 409, error: "key_already_revoked" | P1 | Abnormal |
| AKEY-A-003 | Invalid PEM | Register with invalid PEM format | Active SA | 1. POST with public_key_pem="not-a-valid-pem" | 400 Bad Request, error: "invalid_public_key_pem" | P0 | Abnormal |
| AKEY-A-004 | Wrong Algorithm | Register RSA key as ed25519 | Active SA | 1. POST with algorithm="ed25519" but RSA PEM | 400 Bad Request, error: "key_algorithm_mismatch" | P1 | Abnormal |
| AKEY-A-005 | Expired Key | Register key with expires_at in the past | Active SA | 1. POST with expires_at = yesterday | 400 Bad Request, error: "key_already_expired" | P1 | Abnormal |
| AKEY-A-006 | Suspended SA | Register key for suspended SA | SA status=suspended | 1. POST to register key | 400 or 403, error: "service_account_suspended" | P0 | Abnormal |
| AKEY-A-007 | Revoked SA | Register key for revoked SA | SA status=revoked | 1. POST to register key | 400 or 403, error: "service_account_revoked" | P0 | Abnormal |
| AKEY-A-008 | Missing Fields | Register key without key_id | Active SA | 1. POST without key_id field | 400 Bad Request, error: "missing_field: key_id" | P1 | Abnormal |
| AKEY-A-009 | Missing Fields | Register key without public_key_pem | Active SA | 1. POST without public_key_pem | 400 Bad Request, error: "missing_field: public_key_pem" | P1 | Abnormal |
| AKEY-A-010 | Empty PEM | Register key with empty PEM string | Active SA | 1. POST with public_key_pem="" | 400 Bad Request, error: "public_key_pem_empty" | P1 | Abnormal |

---

## 2.12 Admin API -- Policies

### 2.12.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| APOL-N-001 | Create | Create policy with all fields | Active tenant | 1. POST /admin/tenants/{tid}/policies with name, allowed_models, max_input_tokens, etc. | 201 Created, policy with all fields set | P0 | Normal |
| APOL-N-002 | List | List policies for tenant | Policies exist | 1. GET /admin/tenants/{tid}/policies | 200 OK, array of policies | P0 | Normal |
| APOL-N-003 | Get | Get policy by ID | Policy exists | 1. GET /admin/policies/{id} | 200 OK, full policy details | P0 | Normal |
| APOL-N-004 | Update | Update policy allowed_models | Policy exists | 1. PATCH with new allowed_models_json | 200 OK, allowed_models updated, audit event logged | P0 | Normal |
| APOL-N-005 | Minimal | Create policy with only required fields | Active tenant | 1. POST with just name | 201 Created, optional fields use defaults | P1 | Normal |
| APOL-N-006 | Budget | Create policy with budget limits | Active tenant | 1. POST with daily_budget=100.00, monthly_budget=1000.00 | 201 Created with budget fields | P1 | Normal |
| APOL-N-007 | Features | Create policy with feature flags | Active tenant | 1. POST with allow_streaming=true, allow_tools=true, allow_files=false | 201 Created with correct flags | P1 | Normal |

### 2.12.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| APOL-A-001 | Conflict | Conflicting allow and deny lists | Active tenant | 1. POST with allowed_models=["gpt-4"] and denied_models=["gpt-4"] | 400 Bad Request, error: "conflicting_model_lists" or accepted with deny-takes-precedence semantic | P1 | Abnormal |
| APOL-A-002 | Validation | Negative max_input_tokens | Active tenant | 1. POST with max_input_tokens=-100 | 400 Bad Request, error: "invalid_max_input_tokens" | P1 | Abnormal |
| APOL-A-003 | Validation | Negative max_output_tokens | Active tenant | 1. POST with max_output_tokens=-50 | 400 Bad Request, error: "invalid_max_output_tokens" | P1 | Abnormal |
| APOL-A-004 | Validation | Zero RPM limit | Active tenant | 1. POST with rpm_limit=0 | 400 Bad Request, error: "invalid_rpm_limit" (or accepted if 0 means "block all") | P1 | Abnormal |
| APOL-A-005 | Validation | Negative daily budget | Active tenant | 1. POST with daily_budget=-10.00 | 400 Bad Request, error: "invalid_daily_budget" | P1 | Abnormal |
| APOL-A-006 | Validation | Monthly budget less than daily budget | Active tenant | 1. POST with daily_budget=100, monthly_budget=50 | 400 Bad Request, error: "monthly_budget_less_than_daily" or accepted (no cross-field validation) | P2 | Abnormal |
| APOL-A-007 | Validation | Missing name | Active tenant | 1. POST without name field | 400 Bad Request, error: "missing_field: name" | P1 | Abnormal |
| APOL-A-008 | Invalid JSON | Malformed allowed_models_json | Active tenant | 1. POST with allowed_models_json="not-json" | 400 Bad Request, error: "invalid_json: allowed_models" | P1 | Abnormal |
| APOL-A-009 | Cross-Tenant | Access policy from different tenant | Policy in tenant-A | 1. GET /admin/tenants/{tenant-B}/policies/{policy-from-A} | 404 Not Found | P1 | Abnormal |

---

## 2.13 Admin API -- Routes

### 2.13.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AROUT-N-001 | Create | Create provider route | Active tenant | 1. POST /admin/routes with model_alias, provider, provider_model_name, priority | 201 Created, route with enabled=true | P0 | Normal |
| AROUT-N-002 | List | List routes | Routes exist | 1. GET /admin/routes | 200 OK, array of routes | P0 | Normal |
| AROUT-N-003 | List Filter | List routes by model_alias | Routes exist | 1. GET /admin/routes?model_alias=gpt-4 | 200 OK, filtered routes | P1 | Normal |
| AROUT-N-004 | Update Priority | Update route priority | Route exists | 1. PATCH with priority=5 | 200 OK, priority updated, audit event logged | P0 | Normal |
| AROUT-N-005 | Disable | Disable route | Route enabled=true | 1. PATCH with enabled=false | 200 OK, route disabled | P0 | Normal |
| AROUT-N-006 | Enable | Re-enable route | Route enabled=false | 1. PATCH with enabled=true | 200 OK, route enabled | P1 | Normal |
| AROUT-N-007 | Fallback Group | Create routes in same fallback group | Active tenant | 1. Create route-A with fallback_group="group-1" 2. Create route-B with fallback_group="group-1" | Both created, form fallback chain | P1 | Normal |
| AROUT-N-008 | Global Route | Create route without tenant_id (global) | Admin authenticated | 1. POST route with tenant_id=null | 201 Created as global route | P1 | Normal |

### 2.13.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AROUT-A-001 | Duplicate | Duplicate model_alias + provider + priority for same tenant | Route exists | 1. POST with identical model_alias, provider, priority | 409 Conflict or accepted (verify uniqueness constraints) | P1 | Abnormal |
| AROUT-A-002 | Validation | Invalid provider name | Active tenant | 1. POST with provider="nonexistent_provider" | 400 Bad Request, error: "invalid_provider" | P1 | Abnormal |
| AROUT-A-003 | Validation | Zero timeout_ms | Active tenant | 1. POST with timeout_ms=0 | 400 Bad Request, error: "invalid_timeout" | P1 | Abnormal |
| AROUT-A-004 | Validation | Negative priority | Active tenant | 1. POST with priority=-1 | 400 Bad Request, error: "invalid_priority" | P2 | Abnormal |
| AROUT-A-005 | Validation | Negative max_retries | Active tenant | 1. POST with max_retries=-1 | 400 Bad Request, error: "invalid_max_retries" | P2 | Abnormal |
| AROUT-A-006 | Validation | Missing model_alias | Active tenant | 1. POST without model_alias | 400 Bad Request, error: "missing_field: model_alias" | P1 | Abnormal |
| AROUT-A-007 | Validation | Missing provider | Active tenant | 1. POST without provider | 400 Bad Request, error: "missing_field: provider" | P1 | Abnormal |
| AROUT-A-008 | Validation | Missing provider_model_name | Active tenant | 1. POST without provider_model_name | 400 Bad Request, error: "missing_field: provider_model_name" | P1 | Abnormal |
| AROUT-A-009 | Timeout | Extremely large timeout_ms (>300000) | Active tenant | 1. POST with timeout_ms=999999 | 400 Bad Request, error: "timeout_too_large" or accepted with warning | P2 | Abnormal |
| AROUT-A-010 | Not Found | Update non-existent route | Route ID does not exist | 1. PATCH /admin/routes/{random-uuid} | 404 Not Found | P1 | Abnormal |

---

## 2.14 Audit Logging

### 2.14.1 Normal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AUDIT-N-001 | Tenant | Tenant creation logged | Admin creates tenant | 1. Create tenant 2. Query audit_events | Event: action="tenant.created", target_type="tenant", target_id=tenant_id, actor_type/actor_id populated | P0 | Normal |
| AUDIT-N-002 | Tenant | Tenant status change logged | Admin updates tenant status | 1. Update tenant status 2. Query audit_events | Event: action="tenant.status_changed", metadata includes old_status and new_status | P0 | Normal |
| AUDIT-N-003 | SA | Service account creation logged | Admin creates SA | 1. Create SA 2. Query audit_events | Event: action="service_account.created" | P0 | Normal |
| AUDIT-N-004 | SA | Service account status change logged | Admin updates SA | 1. Suspend SA 2. Query audit_events | Event: action="service_account.status_changed" | P0 | Normal |
| AUDIT-N-005 | Key | Key registration logged | Admin registers key | 1. Register key 2. Query audit_events | Event: action="key.registered", no private key material in metadata | P0 | Normal |
| AUDIT-N-006 | Key | Key revocation logged | Admin revokes key | 1. Revoke key 2. Query audit_events | Event: action="key.revoked" with key_id in metadata | P0 | Normal |
| AUDIT-N-007 | Policy | Policy creation logged | Admin creates policy | 1. Create policy 2. Query audit_events | Event: action="policy.created" | P0 | Normal |
| AUDIT-N-008 | Policy | Policy update logged | Admin updates policy | 1. Update policy 2. Query audit_events | Event: action="policy.updated", metadata includes changed fields | P0 | Normal |
| AUDIT-N-009 | Route | Route creation logged | Admin creates route | 1. Create route 2. Query audit_events | Event: action="route.created" | P0 | Normal |
| AUDIT-N-010 | Route | Route update logged | Admin updates route | 1. Update route 2. Query audit_events | Event: action="route.updated" | P0 | Normal |
| AUDIT-N-011 | Metadata | Audit event metadata complete | Any admin action | 1. Perform admin action 2. Check audit event | created_at, tenant_id, actor_type, actor_id, metadata_json all populated | P0 | Normal |
| AUDIT-N-012 | Ordering | Audit events in chronological order | Multiple actions | 1. Perform actions A, B, C 2. Query audit_events ORDER BY created_at | Events returned in correct chronological order | P1 | Normal |

### 2.14.2 Abnormal Cases

| ID | Category | Test Case | Pre-conditions | Steps | Expected Result | Priority | Type |
|----|----------|-----------|----------------|-------|-----------------|----------|------|
| AUDIT-A-001 | Immutability | Attempt to UPDATE audit_events record | Audit event exists | 1. Execute UPDATE on audit_events table directly | Operation fails or is blocked (verify DB-level constraint: trigger, read-only role, or app-level enforcement) | P0 | Abnormal |
| AUDIT-A-002 | Immutability | Attempt to DELETE audit_events record | Audit event exists | 1. Execute DELETE on audit_events table directly | Operation fails or is blocked | P0 | Abnormal |
| AUDIT-A-003 | Completeness | No audit events missed under concurrent admin operations | High admin concurrency | 1. Perform 50 admin operations concurrently 2. Count audit_events | Exactly 50 audit events recorded (1:1 with operations) | P0 | Abnormal |
| AUDIT-A-004 | No Sensitive Data | Audit metadata does not contain secrets | Key registration with PEM | 1. Register key 2. Check audit event metadata | metadata_json does not contain private key material, full PEM not logged | P0 | Abnormal |
| AUDIT-A-005 | Failed Action | Failed admin action still logged | Invalid admin request | 1. Send invalid admin request (e.g., duplicate slug) 2. Check audit | Verify policy: are failed attempts logged? Document actual behavior | P1 | Abnormal |
| AUDIT-A-006 | No Admin Endpoint | No API endpoint to modify audit events | Admin API | 1. Check for PUT/PATCH/DELETE endpoints on /admin/audit-events | No such endpoints exist | P0 | Abnormal |
| AUDIT-A-007 | DB Failure | Audit logging when DB is temporarily unavailable | DB connection lost | 1. Perform admin action during DB outage | Admin action fails (audit is part of transaction) or audit queued for retry | P1 | Abnormal |

---

# Part 3: Non-Functional Test Cases

## 3.1 Performance Tests

### 3.1.1 Latency Tests

| ID | Category | Test Case | Configuration | Expected Result | Priority |
|----|----------|-----------|---------------|-----------------|----------|
| PERF-L-001 | Baseline | Measure p50/p95/p99 latency at 100 concurrent users | k6: 100 VUs, 5min duration, /v1/chat/completions (mock provider, 50ms response) | p95 gateway overhead < 200ms (per PRD NFR 5.1) | P0 |
| PERF-L-002 | Medium Load | Measure latency at 1000 concurrent users | k6: 1000 VUs, 10min duration | p95 overhead < 200ms, no error rate increase | P0 |
| PERF-L-003 | High Load | Measure latency at 10000 concurrent users | k6: 10000 VUs, 10min duration | Establish degradation profile, document p95/p99, error rate | P1 |
| PERF-L-004 | Auth Overhead | Measure auth verification overhead | Isolate auth middleware time via tracing spans | Signature verification < 1ms p95 | P1 |
| PERF-L-005 | Policy Overhead | Measure policy engine overhead | Isolate policy evaluation time | Policy check < 5ms p95 | P1 |
| PERF-L-006 | Cold Start | Measure first request latency after restart | Restart gateway, send first request | First request < 500ms (connection pool warm-up) | P2 |

### 3.1.2 Throughput Tests

| ID | Category | Test Case | Configuration | Expected Result | Priority |
|----|----------|-----------|---------------|-----------------|----------|
| PERF-T-001 | Peak RPS | Determine maximum requests/second | k6: ramp up VUs until error rate > 1% | Document saturation point (target: > 5000 RPS for non-streaming) | P0 |
| PERF-T-002 | Sustained | Sustained throughput over 1 hour | k6: constant 500 VUs, 1 hour | No latency degradation, no memory growth, stable error rate | P0 |
| PERF-T-003 | Streaming | Streaming throughput | k6: 500 concurrent SSE connections | All streams stable, TTFT overhead < 100ms | P0 |

### 3.1.3 Database Performance

| ID | Category | Test Case | Configuration | Expected Result | Priority |
|----|----------|-----------|---------------|-----------------|----------|
| PERF-D-001 | Write | Usage event INSERT performance | 1000 concurrent INSERTs | p95 < 10ms per INSERT | P1 |
| PERF-D-002 | Read | Service account + key lookup | Concurrent reads under load | p95 < 5ms | P1 |
| PERF-D-003 | Index | Query plan analysis for critical queries | EXPLAIN ANALYZE on all indexed queries | All queries use index scans, no sequential scans on large tables | P1 |
| PERF-D-004 | Pool | Connection pool behavior under load | 10000 concurrent requests, pool_size=20 | No connection exhaustion errors, queuing works correctly | P1 |

### 3.1.4 Redis Performance

| ID | Category | Test Case | Configuration | Expected Result | Priority |
|----|----------|-----------|---------------|-----------------|----------|
| PERF-R-001 | Nonce | Nonce SET + TTL operation | 10000 concurrent nonce checks | p95 < 1ms | P1 |
| PERF-R-002 | Rate Limit | Rate limit INCR + EXPIRE | 10000 concurrent rate limit checks | p95 < 1ms | P1 |
| PERF-R-003 | Memory | Redis memory under sustained load | 1 hour sustained traffic | Memory growth is bounded, TTLs expiring correctly | P1 |

### 3.1.5 Memory and Resource Tests

| ID | Category | Test Case | Configuration | Expected Result | Priority |
|----|----------|-----------|---------------|-----------------|----------|
| PERF-M-001 | Memory Leak | Gateway memory under sustained load | 1 hour sustained 1000 VUs | RSS does not grow unboundedly (< 10% growth after warm-up) | P0 |
| PERF-M-002 | File Descriptors | FD usage under many connections | 5000 concurrent connections | No FD exhaustion, proper cleanup on disconnect | P1 |
| PERF-M-003 | Streaming Memory | Memory during many concurrent streams | 1000 concurrent SSE streams | Memory proportional to stream count, released on completion/disconnect | P1 |

---

## 3.2 Security Tests

### 3.2.1 OWASP API Security Top 10 (2023)

| ID | OWASP Category | Test Case | Steps | Expected Result | Priority |
|----|---------------|-----------|-------|-----------------|----------|
| SEC-API-001 | API1: BOLA | Access another tenant's resources via ID manipulation | 1. Authenticate as tenant-A 2. Request resources using tenant-B's IDs | 403 or 404, no data leakage | P0 |
| SEC-API-002 | API1: BOLA | Access another SA's keys via ID manipulation | 1. Authenticate as SA-1 2. Try to access SA-2's resources | Denied, no cross-SA data access | P0 |
| SEC-API-003 | API2: Broken Auth | Access data plane without auth headers | 1. Send request to /v1/chat/completions with no auth | 401 Unauthorized | P0 |
| SEC-API-004 | API2: Broken Auth | Access admin API without JWT | 1. Send request to /admin/tenants with no Authorization header | 401 Unauthorized | P0 |
| SEC-API-005 | API2: Broken Auth | Use SA credentials for admin endpoints | 1. Use data-plane Ed25519 auth for admin endpoints | 401 or 403, different auth schemes enforced | P0 |
| SEC-API-006 | API3: BOPLA | Excessive data in response (property level) | 1. Create SA 2. GET SA details | Response does not include internal fields (e.g., hashed secrets, internal IDs not meant for consumers) | P1 |
| SEC-API-007 | API3: BOPLA | Mass assignment via extra fields | 1. POST to create SA with extra fields (e.g., is_admin=true, tenant_id=other) | Extra fields ignored, no privilege escalation | P0 |
| SEC-API-008 | API4: Unrestricted Resource Consumption | Request body size limits enforced | 1. Send 100MB JSON body | 413 Payload Too Large, connection terminated quickly | P0 |
| SEC-API-009 | API4: Unrestricted Resource Consumption | Rate limiting prevents resource exhaustion | 1. Send 10000 rapid requests | 429 after limit, server remains responsive | P0 |
| SEC-API-010 | API5: Broken Function Level Auth | Non-admin user accesses admin endpoints | 1. Use data-plane credentials for admin CRUD | 401/403 | P0 |
| SEC-API-011 | API6: SSRF | Server-side request forgery via model/provider fields | 1. Set provider URL to internal service (http://169.254.169.254/) | Request rejected, no internal network access | P0 |
| SEC-API-012 | API7: Security Misconfiguration | Debug endpoints not exposed in production | 1. Probe /debug, /metrics (unauthenticated), /health/detailed | No sensitive info exposed without auth | P1 |
| SEC-API-013 | API7: Security Misconfiguration | Error responses do not leak stack traces | 1. Trigger 500 error 2. Check response body | No stack traces, no internal paths, generic error message | P0 |
| SEC-API-014 | API8: Lack of Protection from Automated Threats | Credential stuffing prevention | 1. Rapidly try many different SA IDs | Rate-limited or blocked after threshold | P1 |
| SEC-API-015 | API9: Improper Inventory Management | Undocumented endpoints not accessible | 1. Fuzz common API paths (/api, /swagger, /graphql, /admin/debug) | 404 for all undocumented paths | P1 |
| SEC-API-016 | API10: Unsafe Consumption of APIs | Provider response injection | 1. Mock provider returns malicious payload (XSS, script injection) 2. Check gateway response | Gateway does not relay unvalidated provider output to admin console | P1 |

### 3.2.2 OWASP Top 10 for LLM Applications (2025)

| ID | LLM Risk | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SEC-LLM-001 | LLM01: Prompt Injection | Prompt injection via user message attempting to override system prompt | 1. Send message with "Ignore all previous instructions..." 2. Check if gateway logs/flags the attempt | Gateway passes to provider (not its responsibility to filter), but logs suspicious patterns if monitoring enabled | P1 |
| SEC-LLM-002 | LLM01: Prompt Injection | Indirect prompt injection via model field manipulation | 1. Set model="gpt-4; DROP TABLE" 2. Check SQL safety | Model field sanitized, no SQL injection | P0 |
| SEC-LLM-003 | LLM02: Sensitive Info Disclosure | Provider error messages not leaked verbatim | 1. Trigger provider error 2. Check gateway error response | Gateway normalizes errors, does not expose provider API keys or internal URLs | P0 |
| SEC-LLM-004 | LLM06: Excessive Agency | Gateway does not auto-execute tool outputs | 1. Send request with tool results 2. Verify gateway only relays | Gateway acts as pass-through, does not autonomously call tools | P1 |
| SEC-LLM-005 | LLM07: System Prompt Leakage | System prompt not logged in usage_events | 1. Send request with system prompt 2. Check usage_events/audit | Full prompt content not stored in metadata unless explicitly configured | P1 |
| SEC-LLM-006 | LLM10: Unbounded Consumption | Token limit enforcement prevents runaway costs | 1. Send request with max_tokens=1000000 2. Check policy enforcement | Policy caps output tokens per max_output_tokens | P0 |
| SEC-LLM-007 | LLM10: Unbounded Consumption | Budget enforcement prevents financial impact | 1. Exhaust budget 2. Send more requests | 403, budget_exhausted error | P0 |

### 3.2.3 Injection Attacks

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SEC-INJ-001 | SQL Injection | SQL injection in tenant slug | 1. POST /admin/tenants with slug="'; DROP TABLE tenants;--" | 400 Bad Request or safely handled (parameterized queries) | P0 |
| SEC-INJ-002 | SQL Injection | SQL injection in service account name | 1. POST with name="' OR '1'='1" | Safely handled via parameterized queries | P0 |
| SEC-INJ-003 | SQL Injection | SQL injection in policy JSONB fields | 1. POST policy with allowed_models_json containing SQL | Safely handled | P0 |
| SEC-INJ-004 | Header Injection | CRLF injection in auth headers | 1. Set X-Service-Account-Id to "id\r\nX-Admin: true" | Header injection prevented | P0 |
| SEC-INJ-005 | Header Injection | Null byte injection in headers | 1. Include \x00 in header values | Rejected or safely handled | P1 |
| SEC-INJ-006 | NoSQL/Redis | Redis command injection via nonce | 1. Set nonce to contain Redis protocol characters | Safely handled, nonce used as simple key | P1 |
| SEC-INJ-007 | JSON Injection | Deeply nested JSON payload | 1. Send JSON with 1000 levels of nesting | 400 Bad Request or handled without stack overflow | P1 |
| SEC-INJ-008 | JSON Injection | JSON with duplicate keys | 1. Send JSON with duplicate "model" keys | Deterministic handling (last wins or first wins), no crash | P2 |

### 3.2.4 Authentication-Specific Security

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SEC-AUTH-001 | Replay | Full request replay within 5-minute window | 1. Capture complete valid request 2. Replay exact same request | 401, nonce_replayed (even within timestamp window) | P0 |
| SEC-AUTH-002 | Replay | Replay after nonce TTL expires (>5 min) | 1. Capture request 2. Wait > 5 min 3. Replay | 401, timestamp_expired (timestamp check catches it) | P0 |
| SEC-AUTH-003 | Timing | Constant-time signature comparison | 1. Measure response time for valid vs invalid signatures across 1000 requests | No statistically significant timing difference (< 1ms variance) | P0 |
| SEC-AUTH-004 | Enumeration | Service account enumeration via error messages | 1. Try various SA IDs 2. Compare error messages for existing vs non-existing | Same error message and response time for both cases | P1 |
| SEC-AUTH-005 | Key Confusion | Use key from SA-A with SA-B's service_account_id | 1. Sign request with SA-A's key 2. Set X-Service-Account-Id to SA-B | 401, signature_invalid (key belongs to different SA) | P0 |
| SEC-AUTH-006 | Downgrade | Attempt to use weaker algorithm | 1. Send algorithm indicator for RSA/HMAC instead of Ed25519 | Rejected, only Ed25519 accepted | P1 |

### 3.2.5 TLS and Transport Security

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SEC-TLS-001 | TLS | Only TLS 1.2+ accepted | 1. Attempt connection with TLS 1.0/1.1 | Connection refused | P0 |
| SEC-TLS-002 | TLS | TLS 1.3 preferred | 1. Connect with TLS 1.3 capable client | TLS 1.3 negotiated | P1 |
| SEC-TLS-003 | HTTP | HTTP (non-TLS) connections rejected | 1. Connect via plain HTTP | Connection refused or redirected to HTTPS | P0 |
| SEC-TLS-004 | Cipher | Weak cipher suites rejected | 1. Attempt with RC4, DES, export ciphers | Handshake fails | P1 |
| SEC-TLS-005 | 0-RTT | TLS 0-RTT disabled (per PRD) | 1. Attempt 0-RTT connection | 0-RTT data rejected | P1 |

### 3.2.6 Header Security

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SEC-HDR-001 | CORS | CORS headers restrict origins | 1. Send request with Origin header from untrusted domain | No Access-Control-Allow-Origin for untrusted origins | P1 |
| SEC-HDR-002 | Clickjacking | X-Frame-Options present on admin console | 1. Check response headers from admin UI | X-Frame-Options: DENY or SAMEORIGIN | P1 |
| SEC-HDR-003 | CSP | Content-Security-Policy on admin console | 1. Check admin UI response headers | CSP header present with restrictive policy | P1 |
| SEC-HDR-004 | HSTS | Strict-Transport-Security present | 1. Check response headers | HSTS header with max-age >= 31536000 | P1 |
| SEC-HDR-005 | Content-Type | X-Content-Type-Options: nosniff | 1. Check response headers | Header present | P2 |
| SEC-HDR-006 | Server | No Server version disclosure | 1. Check response headers | No "Server: Axum/x.x" or similar version info | P1 |

---

## 3.3 Reliability Tests

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| REL-001 | Failover | Provider failover on primary 500 | 1. Configure primary + fallback 2. Make primary return 500 3. Send request | Request served by fallback, user gets 200 OK | P0 |
| REL-002 | Failover | Provider failover on primary timeout | 1. Make primary hang 2. Send request | Request served by fallback after timeout | P0 |
| REL-003 | DB Loss | Data plane survives DB connection loss (short outage) | 1. Drop DB connection for 10s 2. Send data plane requests | Auth may fail (key lookup needs DB), or cached keys work; document behavior | P0 |
| REL-004 | DB Loss | Admin API fails gracefully during DB loss | 1. Drop DB connection 2. Send admin requests | 503 Service Unavailable with clear error, no crash | P0 |
| REL-005 | Redis Loss | Data plane when Redis is unavailable | 1. Stop Redis 2. Send data plane request | Nonce check fails (fail-closed) or degraded mode (fail-open); document and verify policy | P0 |
| REL-006 | Redis Loss | Rate limiting when Redis is unavailable | 1. Stop Redis 2. Send requests | Fail-open (allow) or fail-closed (deny); verify configured behavior | P0 |
| REL-007 | Network | Network partition between gateway and provider | 1. Block outbound traffic to provider 2. Send request | Timeout, then fallback if configured | P1 |
| REL-008 | Shutdown | Graceful shutdown completes in-flight requests | 1. Start 10 concurrent requests 2. Send SIGTERM to gateway | All in-flight requests complete, no new requests accepted, clean exit | P0 |
| REL-009 | Shutdown | Graceful shutdown closes streaming connections | 1. Start streaming request 2. Send SIGTERM | Stream receives error event, then connection closes cleanly | P1 |
| REL-010 | Recovery | Gateway recovers after crash (OOM, panic) | 1. Cause crash 2. Restart 3. Send request | Gateway starts cleanly, serves requests, no data corruption | P0 |
| REL-011 | Recovery | PostgreSQL reconnect after restart | 1. Restart PostgreSQL 2. Send request | Gateway reconnects via connection pool, request succeeds | P1 |
| REL-012 | Recovery | Redis reconnect after restart | 1. Restart Redis 2. Send request | Gateway reconnects, nonce/rate-limit operations resume | P1 |
| REL-013 | Partial Failure | One provider adapter panics, others unaffected | 1. Trigger panic in one adapter 2. Send request to different provider | Other providers work normally, panicked adapter handled safely | P1 |

---

## 3.4 Scalability Tests

| ID | Category | Test Case | Steps | Expected Result | Priority |
|----|----------|-----------|-------|-----------------|----------|
| SCALE-001 | Horizontal | Multiple gateway instances behind load balancer | 1. Deploy 3 instances 2. Send requests | Requests distributed, all instances serve correctly | P0 |
| SCALE-002 | Consistency | Nonce uniqueness across instances | 1. Deploy 3 instances 2. Same nonce to different instances | Nonce rejected on second attempt (Redis shared) | P0 |
| SCALE-003 | Consistency | Rate limits consistent across instances | 1. Deploy 3 instances 2. Distribute requests | Total rate limit enforced globally via Redis | P0 |
| SCALE-004 | DB Pool | Database connection pool exhaustion | 1. Set pool_size=5 2. Send 100 concurrent requests needing DB | Requests queue for connection, no errors (may see increased latency) | P1 |
| SCALE-005 | Redis Pool | Redis connection pool exhaustion | 1. Set Redis pool_size=5 2. Send 100 concurrent requests | Redis operations queue, no errors | P1 |
| SCALE-006 | Data Growth | Performance with large tenant count | 1. Create 1000 tenants 2. Measure admin API list performance | List with pagination still responsive (< 200ms) | P2 |
| SCALE-007 | Data Growth | Performance with large usage_events table | 1. Insert 10M usage events 2. Query usage by tenant | Query time acceptable with indexes (< 500ms) | P2 |
| SCALE-008 | Key Lookup | Key lookup performance with many keys per SA | 1. Register 100 keys per SA 2. Authenticate | Key lookup still < 5ms | P2 |

---

# Part 4: Frontend (Admin Console) Test Cases

## 4.1 Authentication

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-AUTH-001 | Login | Successful login with valid credentials | 1. Navigate to /login 2. Enter valid admin credentials 3. Submit | Redirected to dashboard, session created | P0 | Normal |
| FE-AUTH-002 | Login | Login with invalid credentials | 1. Enter wrong password 2. Submit | Error message displayed, no redirect | P0 | Abnormal |
| FE-AUTH-003 | Logout | Successful logout | 1. Click logout 2. Attempt to access protected page | Redirected to login, session cleared | P0 | Normal |
| FE-AUTH-004 | Session | Session expiry handled | 1. Wait for session to expire 2. Navigate | Redirected to login with "session expired" message | P1 | Abnormal |
| FE-AUTH-005 | Guard | Protected route without auth | 1. Navigate directly to /admin/tenants without login | Redirected to /login | P0 | Abnormal |

## 4.2 Tenant Management

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-TNT-001 | Create | Create tenant via form | 1. Navigate to Tenants 2. Click Create 3. Fill name+slug 4. Submit | Tenant created, appears in list, success toast | P0 | Normal |
| FE-TNT-002 | List | Tenant list displays correctly | 1. Navigate to Tenants | Table shows name, slug, status, created_at with correct data | P0 | Normal |
| FE-TNT-003 | Detail | View tenant details | 1. Click on tenant in list | Detail page shows all fields, associated SAs | P0 | Normal |
| FE-TNT-004 | Update | Update tenant status | 1. Open tenant detail 2. Change status 3. Save | Status updated, success message | P0 | Normal |
| FE-TNT-005 | Validation | Submit create form with empty fields | 1. Click Create 2. Submit without filling fields | Form validation errors displayed inline | P1 | Abnormal |
| FE-TNT-006 | Error | Duplicate slug error displayed | 1. Try to create tenant with existing slug | API error shown as user-friendly message | P1 | Abnormal |

## 4.3 Service Account Management

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-SA-001 | Create | Create SA via form | 1. Navigate to SA 2. Fill form 3. Submit | SA created, success toast | P0 | Normal |
| FE-SA-002 | List | SA list filtered by tenant | 1. Select tenant 2. View SAs | Only SAs for selected tenant shown | P0 | Normal |
| FE-SA-003 | Status | Suspend SA via UI | 1. Open SA detail 2. Click Suspend 3. Confirm | Status changes to suspended | P0 | Normal |
| FE-SA-004 | Validation | Create SA with invalid environment | 1. Enter invalid environment value | Validation error shown | P1 | Abnormal |

## 4.4 Key Management

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-KEY-001 | Register | Register key via form | 1. Navigate to SA keys 2. Paste PEM 3. Submit | Key registered, appears in list | P0 | Normal |
| FE-KEY-002 | Revoke | Revoke key via UI | 1. Click Revoke on key 2. Confirm dialog | Key status changes to revoked, confirmation dialog prevents accidental revocation | P0 | Normal |
| FE-KEY-003 | Display | Key list shows status and expiry | 1. View key list | Columns: key_id, algorithm, status, expires_at, last_used_at | P1 | Normal |
| FE-KEY-004 | Invalid PEM | Paste invalid PEM in form | 1. Paste "random text" as PEM 2. Submit | Validation error: "Invalid PEM format" | P1 | Abnormal |

## 4.5 Policy and Route Management

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-POL-001 | Create | Create policy with all fields | 1. Fill policy form 2. Submit | Policy created with all configured fields | P0 | Normal |
| FE-POL-002 | Edit | Edit policy allowed models | 1. Open policy 2. Modify allowed_models 3. Save | Policy updated | P0 | Normal |
| FE-RTE-001 | Create | Create route via form | 1. Fill route form 2. Submit | Route created | P0 | Normal |
| FE-RTE-002 | Toggle | Enable/disable route via toggle | 1. Click toggle on route row | Route status changes immediately | P0 | Normal |
| FE-RTE-003 | Priority | Drag to reorder priorities | 1. Drag route to new position | Priority values updated | P2 | Normal |

## 4.6 General UI Quality

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| FE-UI-001 | Loading | Loading states shown during API calls | 1. Trigger any CRUD operation 2. Observe UI | Spinner/skeleton shown while loading, disabled buttons prevent double-submit | P1 | Normal |
| FE-UI-002 | Error | API error displayed as toast/alert | 1. Trigger server error (e.g., 500) | User-friendly error message displayed, not raw JSON | P0 | Abnormal |
| FE-UI-003 | Empty State | Empty state for lists with no data | 1. View empty tenant list | "No tenants found" message with create CTA | P2 | Normal |
| FE-UI-004 | Responsive | Admin console on mobile viewport | 1. Resize to 375px width | Layout adapts, navigation collapses, tables scroll horizontally | P2 | Normal |
| FE-UI-005 | Responsive | Admin console on tablet viewport | 1. Resize to 768px width | Layout adapts appropriately | P2 | Normal |
| FE-UI-006 | a11y | Keyboard navigation | 1. Navigate entire UI using Tab/Enter/Escape | All interactive elements reachable and operable via keyboard | P1 | Normal |
| FE-UI-007 | a11y | Screen reader compatibility | 1. Navigate with screen reader (VoiceOver/NVDA) | ARIA labels present, form labels associated, landmarks defined | P1 | Normal |
| FE-UI-008 | a11y | Color contrast | 1. Audit with axe-core | No contrast ratio violations (WCAG 2.1 AA minimum 4.5:1) | P2 | Normal |
| FE-UI-009 | Browser | Chrome compatibility | 1. Test all flows in Chrome (latest) | All features work correctly | P0 | Normal |
| FE-UI-010 | Browser | Firefox compatibility | 1. Test all flows in Firefox (latest) | All features work correctly | P1 | Normal |
| FE-UI-011 | Browser | Safari compatibility | 1. Test all flows in Safari (latest) | All features work correctly, SSE handling works | P1 | Normal |
| FE-UI-012 | Browser | Edge compatibility | 1. Test all flows in Edge (latest) | All features work correctly | P2 | Normal |

---

# Part 5: SDK Test Cases

## 5.1 Python SDK -- Client Initialization

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-INIT-001 | Sync | Create sync client | 1. `client = GatewayClient(base_url, sa_id, key_id, private_key)` | Client initialized without error | P0 | Normal |
| SDK-INIT-002 | Async | Create async client | 1. `client = AsyncGatewayClient(...)` | Async client initialized | P0 | Normal |
| SDK-INIT-003 | Missing | Client without required params | 1. `GatewayClient(base_url=None)` | Raises `ValueError` with clear message | P1 | Abnormal |
| SDK-INIT-004 | Invalid Key | Client with invalid private key | 1. Pass garbage string as private_key | Raises `ValueError: invalid_private_key` | P1 | Abnormal |

## 5.2 Python SDK -- Request Signing

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-SIGN-001 | Canonical | Canonical string constructed correctly | 1. Build request 2. Inspect canonical string | Format: METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_SHA256 | P0 | Normal |
| SDK-SIGN-002 | SHA256 | Body SHA256 computed correctly | 1. Hash known body 2. Compare with expected | SHA256 matches `hashlib.sha256(body).hexdigest()` | P0 | Normal |
| SDK-SIGN-003 | Signature | Ed25519 signature verifiable with public key | 1. Sign canonical string 2. Verify with public key | Verification passes | P0 | Normal |
| SDK-SIGN-004 | Headers | All auth headers set in request | 1. Send request 2. Inspect outgoing headers | X-Service-Account-Id, X-Key-Id, X-Timestamp, X-Nonce, X-Body-SHA256, X-Signature all present | P0 | Normal |
| SDK-SIGN-005 | Nonce | Each request gets unique nonce | 1. Send 100 requests 2. Collect nonces | All 100 nonces unique | P0 | Normal |
| SDK-SIGN-006 | Timestamp | Timestamp is current UTC | 1. Send request 2. Check X-Timestamp | Within 1 second of current UTC time | P1 | Normal |
| SDK-SIGN-007 | Empty Body | GET request signs empty body correctly | 1. GET /v1/models 2. Check Body-SHA256 | SHA256 of empty string | P1 | Normal |

## 5.3 Python SDK -- Chat Completions

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-CHAT-001 | Sync | Synchronous chat completion | 1. `resp = client.chat.completions.create(model="gpt-4", messages=[...])` | Response object with choices and usage | P0 | Normal |
| SDK-CHAT-002 | Async | Async chat completion | 1. `resp = await async_client.chat.completions.create(...)` | Response object returned | P0 | Normal |
| SDK-CHAT-003 | Params | Pass temperature, max_tokens, top_p | 1. Create with additional params | Params forwarded, response received | P1 | Normal |
| SDK-CHAT-004 | Error | Handle 401 response | 1. Use invalid credentials 2. Call create() | Raises `AuthenticationError` with message | P0 | Abnormal |
| SDK-CHAT-005 | Error | Handle 429 response | 1. Exceed rate limit 2. Call create() | Raises `RateLimitError` with retry_after info | P0 | Abnormal |
| SDK-CHAT-006 | Error | Handle 500 response | 1. Server error 2. Call create() | Raises `ServerError` | P1 | Abnormal |
| SDK-CHAT-007 | Error | Handle network timeout | 1. Set very low timeout 2. Call create() | Raises `TimeoutError` | P1 | Abnormal |

## 5.4 Python SDK -- Streaming

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-STRM-001 | Sync | Sync streaming iteration | 1. `for chunk in client.chat.completions.create(stream=True, ...):` | Yields chunk objects with delta content | P0 | Normal |
| SDK-STRM-002 | Async | Async streaming iteration | 1. `async for chunk in await async_client.chat.completions.create(stream=True, ...):` | Yields chunk objects | P0 | Normal |
| SDK-STRM-003 | Complete | Stream collects all deltas | 1. Iterate all chunks 2. Concatenate content | Full response reconstructed | P0 | Normal |
| SDK-STRM-004 | Cancel | Cancel stream mid-iteration | 1. Break from iteration after 3 chunks | No error, resources cleaned up | P1 | Abnormal |
| SDK-STRM-005 | Error | Stream error mid-iteration | 1. Server sends error event during stream | Raises `StreamError` with context | P1 | Abnormal |

## 5.5 Python SDK -- Retry Logic

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-RETRY-001 | Retry | Retry on 500 succeeds on second attempt | 1. Mock: first call returns 500, second returns 200 2. Call create() | Response returned successfully, one retry performed | P0 | Normal |
| SDK-RETRY-002 | Retry | Retry on 429 with Retry-After | 1. Mock: 429 with Retry-After: 2 2. Call create() | SDK waits ~2s then retries | P0 | Normal |
| SDK-RETRY-003 | Max Retries | Exhausts max retries | 1. Mock: always returns 500 2. Call create() with max_retries=3 | Raises error after 3 attempts | P1 | Abnormal |
| SDK-RETRY-004 | No Retry | No retry on 400 | 1. Mock: returns 400 2. Call create() | Error raised immediately, no retry | P1 | Normal |
| SDK-RETRY-005 | No Retry | No retry on 401 | 1. Mock: returns 401 2. Call create() | AuthenticationError raised immediately, no retry | P0 | Normal |
| SDK-RETRY-006 | Backoff | Exponential backoff between retries | 1. Mock: returns 500 3 times 2. Measure retry timing | Increasing delays between retries | P1 | Normal |

## 5.6 Python SDK -- Embeddings and Responses

| ID | Category | Test Case | Steps | Expected Result | Priority | Type |
|----|----------|-----------|-------|-----------------|----------|------|
| SDK-EMB-001 | Sync | Create embedding | 1. `resp = client.embeddings.create(model="text-embedding-3-small", input="Hello")` | Response with embedding vector | P0 | Normal |
| SDK-EMB-002 | Batch | Batch embedding | 1. Pass input=["text1", "text2"] | Response with multiple embeddings | P1 | Normal |
| SDK-RSP-001 | Sync | Create response via /v1/responses | 1. `resp = client.responses.create(model="gpt-4", input="Hello")` | Response object | P0 | Normal |
| SDK-RSP-002 | Stream | Stream response | 1. `for event in client.responses.create(stream=True, ...):` | Events yielded | P0 | Normal |

---

# Part 6: Checklists

## 6.1 Pre-deployment Checklist

- [ ] All database migrations applied and verified
- [ ] PostgreSQL connection pool configured (min/max connections)
- [ ] Redis connection configured and verified
- [ ] Provider API keys configured and validated (test call to each provider)
- [ ] TLS certificates installed and valid (not expiring within 30 days)
- [ ] Admin JWT signing key configured (not a default/test value)
- [ ] Environment variables validated (no test values in production)
- [ ] Payload size limits configured (request body max size)
- [ ] Rate limit defaults configured (global, per-tenant, per-SA)
- [ ] Log level set appropriately (INFO in production, not DEBUG)
- [ ] Health check endpoint responding (/health)
- [ ] Readiness check endpoint responding (/ready)
- [ ] DNS configured for gateway and admin console
- [ ] CORS origins configured (not wildcard in production)
- [ ] Graceful shutdown timeout configured
- [ ] Resource limits set (CPU, memory) in container/orchestrator
- [ ] Backup and restore procedures tested for PostgreSQL
- [ ] Redis persistence configured if needed (or accepted as ephemeral)
- [ ] Rollback plan documented and tested
- [ ] Feature flags / kill switches documented

## 6.2 Security Review Checklist

### Authentication and Authorization
- [ ] Ed25519 signature verification uses constant-time comparison
- [ ] Timestamp window enforcement active (5-minute window)
- [ ] Nonce replay protection active (Redis-backed, 300s TTL)
- [ ] Revoked keys cannot authenticate
- [ ] Expired keys cannot authenticate
- [ ] Suspended service accounts cannot authenticate
- [ ] Suspended tenants block all child SA access
- [ ] Admin API requires separate JWT authentication
- [ ] Data plane credentials cannot access admin endpoints
- [ ] Admin JWT tokens have appropriate expiry (short-lived)
- [ ] Key rotation tested and documented

### Data Protection
- [ ] No secrets in logs (API keys, private keys, PEM data)
- [ ] Error messages do not leak internal details (stack traces, file paths, DB schemas)
- [ ] Provider API keys stored encrypted at rest
- [ ] Audit log metadata does not contain sensitive request content
- [ ] No PII logged unless explicitly configured
- [ ] Database connections use TLS (sslmode=require or verify-full)

### Input Validation
- [ ] All user input parameterized in SQL queries (no string concatenation)
- [ ] JSON body size limits enforced before parsing
- [ ] JSON nesting depth limited
- [ ] Header values validated and sanitized
- [ ] Model name validated against allowed character set
- [ ] Slug format validated (alphanumeric + hyphen only)
- [ ] UUID format validated for all ID parameters

### Network Security
- [ ] TLS 1.2+ enforced, 1.0/1.1 disabled
- [ ] TLS 0-RTT disabled
- [ ] Weak cipher suites disabled
- [ ] HSTS header configured
- [ ] CORS restrictive (not wildcard)
- [ ] X-Frame-Options set for admin console
- [ ] Content-Security-Policy configured for admin console
- [ ] X-Content-Type-Options: nosniff set
- [ ] Server version header suppressed
- [ ] Rate limiting active on all endpoints

### OWASP API Security Top 10 (2023)
- [ ] API1: BOLA -- tenant/SA isolation verified, no cross-tenant access
- [ ] API2: Broken Auth -- all endpoints require authentication
- [ ] API3: BOPLA -- responses contain only intended fields, mass assignment prevented
- [ ] API4: Unrestricted Resource Consumption -- body size limits, rate limits, token limits
- [ ] API5: Broken Function Level Auth -- admin vs data plane separation
- [ ] API6: SSRF -- no user-controlled URLs used for outbound requests (except configured providers)
- [ ] API7: Security Misconfiguration -- debug endpoints disabled, default credentials removed
- [ ] API8: Lack of Protection from Automated Threats -- rate limiting, nonce protection
- [ ] API9: Improper Inventory Management -- only documented endpoints accessible
- [ ] API10: Unsafe Consumption -- provider responses validated before relay

### OWASP Top 10 for LLM Applications (2025)
- [ ] LLM01: Prompt Injection -- gateway logs suspicious patterns if monitoring enabled
- [ ] LLM02: Sensitive Info -- provider errors normalized, no credential leakage
- [ ] LLM03: Supply Chain -- dependencies audited (cargo-audit, npm audit, pip-audit)
- [ ] LLM06: Excessive Agency -- gateway is pass-through, no autonomous tool execution
- [ ] LLM07: System Prompt Leakage -- prompts not stored in accessible logs
- [ ] LLM10: Unbounded Consumption -- token limits and budgets enforced

### Dependency Security
- [ ] `cargo audit` passes with zero known vulnerabilities
- [ ] `npm audit` passes for admin console
- [ ] `pip-audit` passes for Python SDK
- [ ] No dependencies with known CVEs in use
- [ ] Dependency lockfiles (Cargo.lock, package-lock.json, requirements.txt) committed

## 6.3 Performance Review Checklist

### Latency
- [ ] p50 gateway overhead measured and documented
- [ ] p95 gateway overhead < 200ms at target load (per PRD)
- [ ] p99 gateway overhead measured and documented
- [ ] Streaming time-to-first-token overhead < 100ms
- [ ] Auth verification overhead < 1ms p95
- [ ] Policy evaluation overhead < 5ms p95
- [ ] Routing resolution overhead < 5ms p95

### Throughput
- [ ] Maximum RPS established with mock provider
- [ ] Sustained throughput stable over 1 hour
- [ ] No latency degradation under sustained load
- [ ] Streaming throughput tested (concurrent SSE connections)

### Resources
- [ ] No memory leaks under sustained load (1 hour test)
- [ ] File descriptor usage bounded
- [ ] CPU usage proportional to load
- [ ] Database connection pool size adequate (no exhaustion under load)
- [ ] Redis connection pool size adequate

### Database
- [ ] All critical queries use index scans (EXPLAIN ANALYZE verified)
- [ ] Usage event INSERT performance < 10ms p95
- [ ] Key lookup performance < 5ms p95
- [ ] No N+1 query patterns
- [ ] Connection pooling configured and tested (PgBouncer or built-in)

### Baseline
- [ ] Performance baseline established for current version
- [ ] Performance regression detection automated in CI (optional but recommended)
- [ ] Load test scripts committed to repository
- [ ] Load test results archived per release

## 6.4 Observability Checklist

### Logging
- [ ] Structured logging enabled (JSON format)
- [ ] Request ID present in all log lines for a request lifecycle
- [ ] Trace ID present for distributed tracing correlation
- [ ] Log levels appropriate (no DEBUG in production)
- [ ] No sensitive data in logs (keys, tokens, PII, request bodies unless configured)
- [ ] Log rotation/retention configured
- [ ] Logs shipped to centralized logging system (ELK, Loki, etc.)

### Metrics
- [ ] Request count metric (by endpoint, status code, model, provider)
- [ ] Request latency histogram (by endpoint, provider)
- [ ] Error rate metric (by error type, provider)
- [ ] Rate limit hit counter
- [ ] Provider success/failure rate
- [ ] Retry count metric
- [ ] Fallback trigger count
- [ ] Token usage metrics (prompt, completion, by model)
- [ ] Cost metrics (by tenant, SA, model, provider)
- [ ] Budget utilization percentage
- [ ] Active connections gauge (total, streaming)
- [ ] Database connection pool utilization
- [ ] Redis connection pool utilization
- [ ] Metrics exported to Prometheus endpoint (/metrics)

### Tracing
- [ ] Distributed tracing enabled (OpenTelemetry)
- [ ] Span hierarchy covers full request lifecycle:
  - [ ] gateway.request (root)
  - [ ] auth.verify
  - [ ] policy.evaluate
  - [ ] routing.resolve
  - [ ] provider.call
  - [ ] usage.persist
- [ ] Trace context propagated to providers where supported
- [ ] Trace sampling rate configured appropriately
- [ ] Traces exported to tracing backend (Jaeger, Tempo, etc.)

### Dashboards
- [ ] Gateway overview dashboard (RPS, latency, error rate)
- [ ] Provider health dashboard (per-provider success rate, latency)
- [ ] Tenant usage dashboard (tokens, cost, request count)
- [ ] Rate limiting dashboard (hits, rejections)
- [ ] Budget monitoring dashboard (utilization, approaching exhaustion)
- [ ] Infrastructure dashboard (CPU, memory, connections, FDs)

### Alerting
- [ ] Alert: error rate > threshold (e.g., > 5% for 5 minutes)
- [ ] Alert: p95 latency > SLO (e.g., > 500ms for 5 minutes)
- [ ] Alert: all providers down for a model alias
- [ ] Alert: database connection pool near exhaustion (> 80%)
- [ ] Alert: Redis connection failure
- [ ] Alert: budget utilization > 90% for any tenant
- [ ] Alert: certificate expiry within 14 days
- [ ] Alert: high rate-limit rejection rate (possible attack or misconfiguration)
- [ ] Alert: disk space low on database server
- [ ] Runbooks linked to each alert

## 6.5 Release Readiness Checklist

### Testing
- [ ] Unit test suite passes (100% P0, >= 95% P1)
- [ ] Integration test suite passes
- [ ] System test suite passes (all critical paths)
- [ ] E2E test suite passes (admin console + SDK)
- [ ] Performance test results within SLO
- [ ] Security scan completed (OWASP ZAP, dependency audit)
- [ ] No P0/P1 defects open
- [ ] All P2 defects triaged and accepted for release

### Documentation
- [ ] API changelog updated
- [ ] OpenAPI spec updated (data plane + admin)
- [ ] SDK changelog updated
- [ ] Admin console changelog updated
- [ ] Migration guide for breaking changes (if any)
- [ ] Runbooks updated for new features/behaviors

### Deployment
- [ ] Docker images built and tagged
- [ ] Kubernetes manifests / Helm charts updated
- [ ] Database migrations tested on staging data (forward + rollback)
- [ ] Redis schema changes documented (new key patterns, TTL changes)
- [ ] Feature flags configured for gradual rollout if needed
- [ ] Canary deployment plan documented
- [ ] Rollback procedure documented and tested
- [ ] Blue-green or canary deployment verified on staging

### Operational
- [ ] Monitoring dashboards verified with staging traffic
- [ ] Alerts verified (fire and resolve cycle on staging)
- [ ] On-call team briefed on changes
- [ ] Incident response plan updated if architecture changed
- [ ] Capacity planning reviewed (expected traffic increase?)
- [ ] Third-party provider SLAs reviewed (rate limits, quotas)

### Sign-off
- [ ] QA sign-off
- [ ] Security review sign-off
- [ ] Performance review sign-off
- [ ] Product owner sign-off
- [ ] SRE/Ops sign-off

---

# Appendix A: Traceability Matrix

This matrix maps test cases to PRD requirements and system modules.

| PRD Section | Module | Test Case IDs |
|-------------|--------|---------------|
| 4.1 API Layer | API | CHAT-N-*, CHAT-A-*, STRM-N-*, STRM-A-*, EMB-N-*, EMB-A-* |
| 4.2 Auth & Security | Auth | AUTH-N-*, AUTH-A-*, SEC-AUTH-* |
| 4.3 Service Accounts | Admin-SA | ASA-N-*, ASA-A-* |
| 4.4 Policy Engine | Policy | POL-N-*, POL-A-*, APOL-N-*, APOL-A-* |
| 4.5 Routing Engine | Routing | ROUTE-N-*, ROUTE-A-*, AROUT-N-*, AROUT-A-* |
| 4.6 Provider Adapters | Providers | CHAT-N-009, CHAT-A-015 through CHAT-A-017, STRM-N-007 |
| 4.7 Streaming Engine | Streaming | STRM-N-*, STRM-A-* |
| 4.8 Usage & Cost | Usage | USAGE-N-*, USAGE-A-* |
| 4.9 Rate Limiting | Rate Limit | RLIM-N-*, RLIM-A-* |
| 4.11 Audit Logging | Audit | AUDIT-N-*, AUDIT-A-* |
| 5.1 Performance | NFR | PERF-L-*, PERF-T-*, PERF-D-*, PERF-R-*, PERF-M-* |
| 5.2 Scalability | NFR | SCALE-* |
| 5.3 Reliability | NFR | REL-* |
| 5.4 Security | NFR | SEC-API-*, SEC-LLM-*, SEC-INJ-*, SEC-TLS-*, SEC-HDR-* |
| 7 SDK | Python SDK | SDK-* |
| 8 Admin Console | Frontend | FE-* |

---

# Appendix B: References

- [OWASP API Security Top 10 (2023)](https://owasp.org/API-Security/editions/2023/en/0x11-t10/)
- [OWASP Top 10 for LLM Applications (2025)](https://genai.owasp.org/resource/owasp-top-10-for-llm-applications-2025/)
- [OWASP API Security Project](https://owasp.org/www-project-api-security/)
- [Grafana k6 Load Testing](https://k6.io/)
- [k6 API Load Testing Guide](https://grafana.com/docs/k6/latest/testing-guides/api-load-testing/)
- [API Gateway Security Best Practices (2026)](https://www.practical-devsecops.com/api-gateway-security-best-practices/)
- [API Testing Complete Guide (2026)](https://totalshiftleft.ai/blog/api-testing-complete-guide)
- [Ed25519 Specification](https://ed25519.cr.yp.to/)
- [LLM Security Risks in 2026](https://sombrainc.com/blog/llm-security-risks-2026)
- [Prompt Injection and LLM API Security](https://www.apisec.ai/blog/prompt-injection-and-llm-api-security-risks-protect-your-ai)
- [OWASP LLM01:2025 Prompt Injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/)
- [F5 API Security Checklist](https://www.f5.com/company/blog/api-security-checklist)
- [API Gateway Security Policy Best Practices (2026)](https://www.saaras.io/blog/api-gateway-security)

---

*End of document.*
