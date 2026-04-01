# LLM Gateway – Low-Level Design (LLD), ERD, SQL Schema, and OpenAPI Skeleton

## 1. Scope

Tài liệu này mô tả:
- Low-Level Design cho từng module của LLM Gateway
- ERD cho control plane và data plane metadata
- SQL schema draft cho PostgreSQL
- OpenAPI skeleton cho Data Plane API và Admin API

Hệ thống mục tiêu:
- Rust backend
- SvelteKit admin console
- Python SDK
- PostgreSQL + Redis
- Asymmetric signing + Service Accounts
- OpenAI-compatible API

---

# 2. Low-Level Design (LLD)

## 2.1 Module: API Layer

### Responsibility
- Expose Data Plane API
- Expose Admin API
- Parse and validate incoming request
- Attach request metadata (`request_id`, `trace_id`, timestamps)
- Orchestrate request lifecycle

### Submodules
- `api::data_plane`
- `api::admin`
- `api::middleware`

### Input
- HTTP request
- Headers
- JSON payload
- Query params

### Output
- JSON response
- SSE/stream response
- Normalized error response

### Internal flow
1. Receive request
2. Generate `request_id`
3. Validate request schema
4. Dispatch:
   - data plane → auth → policy → routing → provider
   - admin plane → admin auth → control plane service
5. Return response

### Error handling
- malformed JSON → `400`
- unsupported route → `404`
- invalid schema → `422`
- internal errors → `500`

### Suggested files
```text
src/api/
  data_plane.rs
  admin.rs
  middleware.rs
  request_context.rs
```

---

## 2.2 Module: Auth

### Responsibility
- Verify asymmetric signature
- Validate timestamp window
- Validate nonce
- Resolve service account and key
- Produce authenticated principal context

### Inputs
Headers:
- `X-Service-Account-Id`
- `X-Key-Id`
- `X-Timestamp`
- `X-Nonce`
- `X-Body-SHA256`
- `X-Signature`

### Outputs
Authenticated context:
```text
tenant_id
service_account_id
key_id
policy_id
auth_method
```

### Internal flow
1. Read required headers
2. Validate mandatory fields
3. Fetch service account
4. Fetch active public key by `key_id`
5. Recompute canonical string
6. Verify signature
7. Check timestamp skew
8. Check nonce uniqueness in Redis
9. Return principal context

### Canonical signing string
```text
HTTP_METHOD
REQUEST_PATH
TIMESTAMP
NONCE
BODY_SHA256
```

### Failure cases
- missing headers
- invalid timestamp
- nonce replay
- key revoked
- signature mismatch
- service account suspended

### Suggested files
```text
src/auth/
  verifier.rs
  canonical.rs
  nonce.rs
  key_store.rs
  context.rs
```

---

## 2.3 Module: Policy Engine

### Responsibility
- Enforce model access control
- Enforce token limits
- Enforce feature flags (streaming, tools, files)
- Enforce per-request and budget guardrails

### Inputs
- authenticated principal
- request payload
- policy definition
- optional budget snapshot
- route alias

### Outputs
- allow
- deny with reason
- allow with normalized constraints

### Internal checks
- model alias allowed?
- denied explicitly?
- input tokens > max?
- requested output tokens > max?
- stream allowed?
- tools allowed?
- file input allowed?
- budget exhausted?
- region restriction violated?

### Suggested files
```text
src/policy/
  engine.rs
  model_access.rs
  quotas.rs
  budgets.rs
  features.rs
```

---

## 2.4 Module: Routing Engine

### Responsibility
- Resolve model alias to provider route
- Select primary provider
- Apply retry policy
- Apply fallback chain
- Return execution plan

### Inputs
- normalized request
- model alias
- tenant/service account context
- route config
- provider health cache

### Outputs
- selected provider target
- retry config
- fallback chain

### Routing v1
- deterministic
- priority-based
- static fallback chain

### Routing v2
- cost-aware
- health-aware
- latency-aware

### Internal flow
1. Resolve alias
2. Filter enabled routes
3. Sort by priority
4. Build attempt plan
5. Return plan to provider execution layer

### Suggested files
```text
src/routing/
  router.rs
  resolver.rs
  retry.rs
  fallback.rs
  health.rs
```

---

## 2.5 Module: Provider Adapters

### Responsibility
- Convert normalized request to provider-specific payload
- Call provider API
- Normalize provider response
- Normalize provider errors
- Support streaming and non-streaming flows

### Core abstraction
```text
NormalizedRequest -> ProviderAdapter -> ProviderResponse -> NormalizedResponse
```

### Adapter responsibilities
For each provider:
- auth header generation
- endpoint selection
- request body transform
- error mapping
- stream parsing

### Common normalized errors
- `provider_timeout`
- `provider_rate_limited`
- `provider_auth_error`
- `provider_unavailable`
- `provider_bad_request`

### Suggested files
```text
src/providers/
  mod.rs
  traits.rs
  openai.rs
  anthropic.rs
  gemini.rs
  azure_openai.rs
  local_vllm.rs
```

---

## 2.6 Module: Streaming Engine

### Responsibility
- Bridge upstream stream to downstream client
- Normalize streaming events
- Detect disconnect
- Cancel upstream on disconnect
- Finalize usage on stream close

### Event model
- `response.started`
- `response.output_text.delta`
- `response.tool_call.delta`
- `response.completed`
- `response.error`

### Internal flow
1. Open upstream stream
2. Emit `response.started`
3. Relay normalized deltas
4. Watch downstream connection
5. On client disconnect:
   - cancel upstream
   - mark partial
6. On completion:
   - emit final event
   - finalize usage

### Suggested files
```text
src/streaming/
  relay.rs
  events.rs
  parser.rs
  cancel.rs
```

---

## 2.7 Module: Usage Metering

### Responsibility
- Record per-request usage
- Estimate cost
- Attribute spend to tenant/service account
- Record retries, provider attempts, latency, status

### Inputs
- request context
- provider response metadata
- tokens
- timing
- model/provider identity

### Outputs
- `usage_events`
- budget updates
- analytics events

### Internal flow
1. Start request timer
2. Accumulate attempt metadata
3. On response complete:
   - compute total duration
   - persist usage event
   - emit analytics event

### Suggested files
```text
src/usage/
  meter.rs
  pricing.rs
  aggregator.rs
```

---

## 2.8 Module: Audit

### Responsibility
- Persist security-sensitive administrative events
- Capture actor, action, target, metadata
- Provide immutable historical trail

### Examples
- service account created
- key registered
- key revoked
- policy changed
- route changed
- provider config updated

### Suggested files
```text
src/audit/
  logger.rs
  event.rs
```

---

## 2.9 Module: Observability

### Responsibility
- Emit metrics
- Emit traces
- Emit structured logs
- Correlate request path end-to-end

### Metrics
- request count
- success/fail rate
- p95/p99 latency
- retry rate
- fallback rate
- provider error rate
- cost per tenant/model/provider

### Traces
Span hierarchy:
- gateway.request
  - auth.verify
  - policy.evaluate
  - routing.resolve
  - provider.call
  - usage.persist

### Suggested files
```text
src/observability/
  metrics.rs
  tracing.rs
  logging.rs
```

---

## 2.10 Module: Admin Control Plane

### Responsibility
- CRUD for tenants
- CRUD for service accounts
- Register/revoke keys
- CRUD policies
- CRUD routes
- Expose audit and usage query APIs

### Suggested files
```text
src/admin/
  tenants.rs
  service_accounts.rs
  keys.rs
  policies.rs
  routes.rs
  usage.rs
  audit.rs
```

---

## 2.11 Module: Storage

### Responsibility
- Encapsulate PostgreSQL access
- Encapsulate Redis access
- Repository/service boundary for persistence

### Suggested files
```text
src/storage/
  postgres.rs
  redis.rs
  repositories/
    tenants_repo.rs
    service_accounts_repo.rs
    keys_repo.rs
    policies_repo.rs
    routes_repo.rs
    usage_repo.rs
    audit_repo.rs
```

---

# 3. Request Flow Details

## 3.1 Data Plane – Non-stream request
1. Client sends signed request
2. API validates JSON and required fields
3. Auth verifies signature and nonce
4. Policy engine checks access
5. Router resolves provider
6. Provider adapter executes request
7. Usage meter stores result
8. API returns normalized JSON

## 3.2 Data Plane – Stream request
1. Same as above until provider execution
2. Streaming engine opens upstream stream
3. Stream events normalized and relayed
4. Client disconnect cancels upstream
5. Usage event persisted at end

## 3.3 Admin Plane – Create service account
1. Admin console sends authenticated admin request
2. Admin API validates payload
3. Service account record created
4. Policy binding optionally added
5. Audit event written
6. Response returned

---

# 4. ERD

## 4.1 Entity overview

```text
tenants
  ├── service_accounts
  │     ├── service_account_keys
  │     ├── service_account_policy_bindings
  │     └── usage_events
  ├── policies
  ├── budgets
  ├── provider_routes
  └── audit_events
```

## 4.2 ERD detail

```text
[tenants]
- id (PK)
- name
- slug
- status
- created_at
- updated_at

[service_accounts]
- id (PK)
- tenant_id (FK -> tenants.id)
- name
- slug
- environment
- description
- status
- default_policy_id (FK -> policies.id, nullable)
- created_at
- updated_at

[service_account_keys]
- id (PK)
- service_account_id (FK -> service_accounts.id)
- key_id (UNIQUE)
- algorithm
- public_key_pem
- fingerprint
- status
- expires_at
- created_at
- revoked_at
- last_used_at

[policies]
- id (PK)
- tenant_id (FK -> tenants.id)
- name
- description
- allowed_models_json
- denied_models_json
- max_input_tokens
- max_output_tokens
- allow_streaming
- allow_tools
- allow_files
- rpm_limit
- concurrency_limit
- daily_budget
- monthly_budget
- allowed_regions_json
- created_at
- updated_at

[service_account_policy_bindings]
- id (PK)
- service_account_id (FK -> service_accounts.id)
- policy_id (FK -> policies.id)
- created_at

[provider_routes]
- id (PK)
- tenant_id (FK -> tenants.id, nullable)
- model_alias
- provider
- provider_model_name
- priority
- enabled
- timeout_ms
- max_retries
- retry_backoff_ms
- fallback_group
- region
- created_at
- updated_at

[budgets]
- id (PK)
- tenant_id (FK -> tenants.id)
- service_account_id (FK -> service_accounts.id, nullable)
- period_type
- amount_limit
- currency
- start_at
- end_at
- status
- created_at
- updated_at

[usage_events]
- id (PK)
- request_id (UNIQUE)
- tenant_id (FK -> tenants.id)
- service_account_id (FK -> service_accounts.id)
- provider_route_id (FK -> provider_routes.id, nullable)
- provider
- model_alias
- provider_model_name
- prompt_tokens
- completion_tokens
- total_tokens
- estimated_cost
- currency
- latency_ms
- retry_count
- final_status
- is_streaming
- created_at

[audit_events]
- id (PK)
- tenant_id (FK -> tenants.id, nullable)
- actor_type
- actor_id
- action
- target_type
- target_id
- metadata_json
- created_at
```

---

# 5. SQL Schema Draft (PostgreSQL)

## 5.1 Extensions
```sql
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
```

## 5.2 Enums
```sql
CREATE TYPE tenant_status AS ENUM ('active', 'suspended', 'deleted');
CREATE TYPE service_account_status AS ENUM ('active', 'suspended', 'revoked');
CREATE TYPE key_status AS ENUM ('active', 'rotating', 'revoked', 'expired');
CREATE TYPE budget_period_type AS ENUM ('daily', 'monthly', 'custom');
CREATE TYPE budget_status AS ENUM ('active', 'exhausted', 'expired', 'disabled');
```

## 5.3 Tenants
```sql
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    status tenant_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## 5.4 Policies
```sql
CREATE TABLE policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    allowed_models_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    denied_models_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    max_input_tokens INTEGER,
    max_output_tokens INTEGER,
    allow_streaming BOOLEAN NOT NULL DEFAULT TRUE,
    allow_tools BOOLEAN NOT NULL DEFAULT FALSE,
    allow_files BOOLEAN NOT NULL DEFAULT FALSE,
    rpm_limit INTEGER,
    concurrency_limit INTEGER,
    daily_budget NUMERIC(18,6),
    monthly_budget NUMERIC(18,6),
    allowed_regions_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_policies_tenant_id ON policies(tenant_id);
```

## 5.5 Service Accounts
```sql
CREATE TABLE service_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    environment TEXT NOT NULL,
    description TEXT,
    status service_account_status NOT NULL DEFAULT 'active',
    default_policy_id UUID REFERENCES policies(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

CREATE INDEX idx_service_accounts_tenant_id ON service_accounts(tenant_id);
CREATE INDEX idx_service_accounts_default_policy_id ON service_accounts(default_policy_id);
```

## 5.6 Service Account Keys
```sql
CREATE TABLE service_account_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_account_id UUID NOT NULL REFERENCES service_accounts(id) ON DELETE CASCADE,
    key_id TEXT NOT NULL UNIQUE,
    algorithm TEXT NOT NULL DEFAULT 'ed25519',
    public_key_pem TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    status key_status NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_service_account_keys_service_account_id
    ON service_account_keys(service_account_id);

CREATE INDEX idx_service_account_keys_status
    ON service_account_keys(status);
```

## 5.7 Service Account Policy Bindings
```sql
CREATE TABLE service_account_policy_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_account_id UUID NOT NULL REFERENCES service_accounts(id) ON DELETE CASCADE,
    policy_id UUID NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (service_account_id, policy_id)
);

CREATE INDEX idx_sapb_service_account_id
    ON service_account_policy_bindings(service_account_id);

CREATE INDEX idx_sapb_policy_id
    ON service_account_policy_bindings(policy_id);
```

## 5.8 Provider Routes
```sql
CREATE TABLE provider_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    model_alias TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_model_name TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    timeout_ms INTEGER NOT NULL DEFAULT 30000,
    max_retries INTEGER NOT NULL DEFAULT 1,
    retry_backoff_ms INTEGER NOT NULL DEFAULT 250,
    fallback_group TEXT,
    region TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_provider_routes_tenant_id ON provider_routes(tenant_id);
CREATE INDEX idx_provider_routes_model_alias ON provider_routes(model_alias);
CREATE INDEX idx_provider_routes_enabled ON provider_routes(enabled);
```

## 5.9 Budgets
```sql
CREATE TABLE budgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    service_account_id UUID REFERENCES service_accounts(id) ON DELETE CASCADE,
    period_type budget_period_type NOT NULL,
    amount_limit NUMERIC(18,6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    status budget_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_budgets_tenant_id ON budgets(tenant_id);
CREATE INDEX idx_budgets_service_account_id ON budgets(service_account_id);
CREATE INDEX idx_budgets_status ON budgets(status);
```

## 5.10 Usage Events
```sql
CREATE TABLE usage_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id TEXT NOT NULL UNIQUE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    service_account_id UUID NOT NULL REFERENCES service_accounts(id) ON DELETE CASCADE,
    provider_route_id UUID REFERENCES provider_routes(id) ON DELETE SET NULL,
    provider TEXT NOT NULL,
    model_alias TEXT NOT NULL,
    provider_model_name TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    estimated_cost NUMERIC(18,6) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    latency_ms INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    final_status TEXT NOT NULL,
    is_streaming BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_usage_events_tenant_id ON usage_events(tenant_id);
CREATE INDEX idx_usage_events_service_account_id ON usage_events(service_account_id);
CREATE INDEX idx_usage_events_created_at ON usage_events(created_at DESC);
CREATE INDEX idx_usage_events_model_alias ON usage_events(model_alias);
CREATE INDEX idx_usage_events_provider ON usage_events(provider);
```

## 5.11 Audit Events
```sql
CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_events_tenant_id ON audit_events(tenant_id);
CREATE INDEX idx_audit_events_action ON audit_events(action);
CREATE INDEX idx_audit_events_created_at ON audit_events(created_at DESC);
```

---

# 6. Redis Key Design

## 6.1 Nonce replay protection
```text
nonce:{service_account_id}:{nonce} = 1
TTL = 300s
```

## 6.2 Rate limit
```text
ratelimit:tenant:{tenant_id}:{window}
ratelimit:sa:{service_account_id}:{window}
```

## 6.3 Session
```text
session:{token_id}
```

## 6.4 Provider health
```text
provider_health:{provider}
```

## 6.5 Route cache
```text
route:{model_alias}
```

---

# 7. Suggested Rust Repository Interfaces

## 7.1 ServiceAccountRepository
```rust
trait ServiceAccountRepository {
    fn get_by_id(&self, id: Uuid) -> Result<ServiceAccount>;
    fn list_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<ServiceAccount>>;
    fn create(&self, input: CreateServiceAccount) -> Result<ServiceAccount>;
    fn update_status(&self, id: Uuid, status: ServiceAccountStatus) -> Result<()>;
}
```

## 7.2 ServiceAccountKeyRepository
```rust
trait ServiceAccountKeyRepository {
    fn get_active_key(&self, key_id: &str) -> Result<ServiceAccountKey>;
    fn list_by_service_account(&self, service_account_id: Uuid) -> Result<Vec<ServiceAccountKey>>;
    fn create(&self, input: CreateServiceAccountKey) -> Result<ServiceAccountKey>;
    fn revoke(&self, key_id: &str) -> Result<()>;
    fn touch_last_used(&self, key_id: &str) -> Result<()>;
}
```

## 7.3 PolicyRepository
```rust
trait PolicyRepository {
    fn get_by_id(&self, id: Uuid) -> Result<Policy>;
    fn list_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Policy>>;
    fn create(&self, input: CreatePolicy) -> Result<Policy>;
}
```

## 7.4 ProviderRouteRepository
```rust
trait ProviderRouteRepository {
    fn list_for_alias(&self, tenant_id: Option<Uuid>, model_alias: &str) -> Result<Vec<ProviderRoute>>;
}
```

---

# 8. Suggested Migrations Order

1. `tenants`
2. `policies`
3. `service_accounts`
4. `service_account_keys`
5. `service_account_policy_bindings`
6. `provider_routes`
7. `budgets`
8. `usage_events`
9. `audit_events`

---

# 9. Notes and Design Decisions

## 9.1 Why JSONB in policies
Dùng `JSONB` cho:
- allow/deny model lists
- allowed regions

Lý do:
- linh hoạt trong giai đoạn đầu
- tránh schema churn quá sớm

Nếu policy complexity tăng mạnh, có thể tách bảng normalization sau.

## 9.2 Why UUID
- tránh lộ sequence
- tiện multi-service generation
- hợp với public-facing identifiers nội bộ

## 9.3 Why separate `service_account_keys`
- hỗ trợ multi-key rotation
- audit tốt hơn
- zero-downtime rotation

## 9.4 Why `usage_events` append-only
- dễ analytics
- dễ audit
- không mất lịch sử khi retry/fallback xảy ra

---

# 10. OpenAPI Skeleton

## 10.1 Data Plane API (OpenAPI 3.1 draft)

```yaml
openapi: 3.1.0
info:
  title: LLM Gateway Data Plane API
  version: 1.0.0
  description: OpenAI-compatible runtime API for chat, responses, embeddings, and models.
servers:
  - url: https://gateway.example.com
tags:
  - name: Chat
  - name: Responses
  - name: Embeddings
  - name: Models
components:
  securitySchemes:
    ServiceAccountSignature:
      type: apiKey
      in: header
      name: X-Signature
  headers:
    X-Service-Account-Id:
      schema: { type: string }
    X-Key-Id:
      schema: { type: string }
    X-Timestamp:
      schema: { type: string, format: date-time }
    X-Nonce:
      schema: { type: string }
    X-Body-SHA256:
      schema: { type: string }
    X-Signature:
      schema: { type: string }
  schemas:
    ErrorResponse:
      type: object
      properties:
        error:
          type: object
          properties:
            code: { type: string }
            message: { type: string }
            request_id: { type: string }
      required: [error]

    Model:
      type: object
      properties:
        id: { type: string }
        object: { type: string, example: model }
        created: { type: integer }
        owned_by: { type: string }
      required: [id, object]

    ModelsListResponse:
      type: object
      properties:
        object: { type: string, example: list }
        data:
          type: array
          items:
            $ref: '#/components/schemas/Model'
      required: [object, data]

    ChatMessage:
      type: object
      properties:
        role:
          type: string
          enum: [system, user, assistant, tool]
        content:
          oneOf:
            - type: string
            - type: array
      required: [role, content]

    ChatCompletionsRequest:
      type: object
      properties:
        model: { type: string }
        messages:
          type: array
          items:
            $ref: '#/components/schemas/ChatMessage'
        temperature: { type: number }
        top_p: { type: number }
        max_tokens: { type: integer }
        stream: { type: boolean, default: false }
      required: [model, messages]

    ChatCompletionsResponse:
      type: object
      properties:
        id: { type: string }
        object: { type: string }
        created: { type: integer }
        model: { type: string }
        choices:
          type: array
          items:
            type: object
        usage:
          type: object
          properties:
            prompt_tokens: { type: integer }
            completion_tokens: { type: integer }
            total_tokens: { type: integer }

    ResponseCreateRequest:
      type: object
      properties:
        model: { type: string }
        input:
          oneOf:
            - type: string
            - type: array
        stream: { type: boolean, default: false }
        max_output_tokens: { type: integer }
      required: [model, input]

    ResponseCreateResponse:
      type: object
      properties:
        id: { type: string }
        object: { type: string, example: response }
        model: { type: string }
        output_text: { type: string }
        usage:
          type: object
          properties:
            input_tokens: { type: integer }
            output_tokens: { type: integer }
            total_tokens: { type: integer }

    EmbeddingsRequest:
      type: object
      properties:
        model: { type: string }
        input:
          oneOf:
            - type: string
            - type: array
      required: [model, input]

    EmbeddingsResponse:
      type: object
      properties:
        object: { type: string, example: list }
        data:
          type: array
          items:
            type: object
            properties:
              object: { type: string, example: embedding }
              index: { type: integer }
              embedding:
                type: array
                items: { type: number }
        model: { type: string }

paths:
  /v1/models:
    get:
      tags: [Models]
      summary: List available model aliases
      responses:
        '200':
          description: Models list
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ModelsListResponse'
        '401':
          description: Unauthorized
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'

  /v1/chat/completions:
    post:
      tags: [Chat]
      summary: Create a chat completion
      parameters:
        - in: header
          name: X-Service-Account-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Key-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Timestamp
          schema: { type: string, format: date-time }
          required: true
        - in: header
          name: X-Nonce
          schema: { type: string }
          required: true
        - in: header
          name: X-Body-SHA256
          schema: { type: string }
          required: true
        - in: header
          name: X-Signature
          schema: { type: string }
          required: true
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ChatCompletionsRequest'
      responses:
        '200':
          description: Chat completion created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ChatCompletionsResponse'
        '400':
          description: Bad request
        '401':
          description: Unauthorized
        '403':
          description: Policy denied
        '429':
          description: Rate limited
        '500':
          description: Internal error

  /v1/responses:
    post:
      tags: [Responses]
      summary: Create a unified response
      parameters:
        - in: header
          name: X-Service-Account-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Key-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Timestamp
          schema: { type: string, format: date-time }
          required: true
        - in: header
          name: X-Nonce
          schema: { type: string }
          required: true
        - in: header
          name: X-Body-SHA256
          schema: { type: string }
          required: true
        - in: header
          name: X-Signature
          schema: { type: string }
          required: true
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ResponseCreateRequest'
      responses:
        '200':
          description: Response created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ResponseCreateResponse'

  /v1/embeddings:
    post:
      tags: [Embeddings]
      summary: Create embeddings
      parameters:
        - in: header
          name: X-Service-Account-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Key-Id
          schema: { type: string }
          required: true
        - in: header
          name: X-Timestamp
          schema: { type: string, format: date-time }
          required: true
        - in: header
          name: X-Nonce
          schema: { type: string }
          required: true
        - in: header
          name: X-Body-SHA256
          schema: { type: string }
          required: true
        - in: header
          name: X-Signature
          schema: { type: string }
          required: true
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/EmbeddingsRequest'
      responses:
        '200':
          description: Embeddings generated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/EmbeddingsResponse'
```

## 10.2 Admin API (OpenAPI 3.1 draft)

```yaml
openapi: 3.1.0
info:
  title: LLM Gateway Admin API
  version: 1.0.0
  description: Control plane API for tenants, service accounts, keys, policies, routes, usage, and audit.
servers:
  - url: https://gateway.example.com
tags:
  - name: Tenants
  - name: Service Accounts
  - name: Keys
  - name: Policies
  - name: Routes
  - name: Usage
  - name: Audit
components:
  securitySchemes:
    AdminBearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
  schemas:
    ErrorResponse:
      type: object
      properties:
        error:
          type: object
          properties:
            code: { type: string }
            message: { type: string }

    Tenant:
      type: object
      properties:
        id: { type: string, format: uuid }
        name: { type: string }
        slug: { type: string }
        status: { type: string }

    CreateTenantRequest:
      type: object
      properties:
        name: { type: string }
        slug: { type: string }
      required: [name, slug]

    ServiceAccount:
      type: object
      properties:
        id: { type: string, format: uuid }
        tenant_id: { type: string, format: uuid }
        name: { type: string }
        slug: { type: string }
        environment: { type: string }
        description: { type: string }
        status: { type: string }

    CreateServiceAccountRequest:
      type: object
      properties:
        tenant_id: { type: string, format: uuid }
        name: { type: string }
        slug: { type: string }
        environment: { type: string }
        description: { type: string }
        default_policy_id: { type: string, format: uuid }
      required: [tenant_id, name, slug, environment]

    RegisterPublicKeyRequest:
      type: object
      properties:
        algorithm: { type: string, example: ed25519 }
        public_key_pem: { type: string }
        expires_at: { type: string, format: date-time }
      required: [algorithm, public_key_pem]

    ServiceAccountKey:
      type: object
      properties:
        id: { type: string, format: uuid }
        key_id: { type: string }
        algorithm: { type: string }
        fingerprint: { type: string }
        status: { type: string }
        expires_at: { type: string, format: date-time }

    Policy:
      type: object
      properties:
        id: { type: string, format: uuid }
        tenant_id: { type: string, format: uuid }
        name: { type: string }
        description: { type: string }
        allowed_models_json: {}
        denied_models_json: {}
        max_input_tokens: { type: integer }
        max_output_tokens: { type: integer }
        allow_streaming: { type: boolean }
        allow_tools: { type: boolean }
        allow_files: { type: boolean }

    CreatePolicyRequest:
      type: object
      properties:
        tenant_id: { type: string, format: uuid }
        name: { type: string }
        description: { type: string }
        allowed_models_json:
          type: array
          items: { type: string }
        denied_models_json:
          type: array
          items: { type: string }
        max_input_tokens: { type: integer }
        max_output_tokens: { type: integer }
        allow_streaming: { type: boolean }
        allow_tools: { type: boolean }
        allow_files: { type: boolean }
      required: [tenant_id, name]

    ProviderRoute:
      type: object
      properties:
        id: { type: string, format: uuid }
        model_alias: { type: string }
        provider: { type: string }
        provider_model_name: { type: string }
        priority: { type: integer }
        enabled: { type: boolean }
        timeout_ms: { type: integer }
        max_retries: { type: integer }
        retry_backoff_ms: { type: integer }
        fallback_group: { type: string }
        region: { type: string }

    CreateProviderRouteRequest:
      type: object
      properties:
        tenant_id: { type: string, format: uuid }
        model_alias: { type: string }
        provider: { type: string }
        provider_model_name: { type: string }
        priority: { type: integer }
        enabled: { type: boolean }
        timeout_ms: { type: integer }
        max_retries: { type: integer }
        retry_backoff_ms: { type: integer }
        fallback_group: { type: string }
        region: { type: string }
      required: [model_alias, provider, provider_model_name]

    UsageEvent:
      type: object
      properties:
        request_id: { type: string }
        tenant_id: { type: string, format: uuid }
        service_account_id: { type: string, format: uuid }
        provider: { type: string }
        model_alias: { type: string }
        provider_model_name: { type: string }
        prompt_tokens: { type: integer }
        completion_tokens: { type: integer }
        total_tokens: { type: integer }
        estimated_cost: { type: number }
        latency_ms: { type: integer }
        retry_count: { type: integer }
        final_status: { type: string }
        created_at: { type: string, format: date-time }

    AuditEvent:
      type: object
      properties:
        id: { type: string, format: uuid }
        tenant_id: { type: string, format: uuid }
        actor_type: { type: string }
        actor_id: { type: string }
        action: { type: string }
        target_type: { type: string }
        target_id: { type: string }
        metadata_json: {}
        created_at: { type: string, format: date-time }

security:
  - AdminBearerAuth: []

paths:
  /admin/tenants:
    get:
      tags: [Tenants]
      summary: List tenants
      responses:
        '200':
          description: Tenant list
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Tenant'
    post:
      tags: [Tenants]
      summary: Create tenant
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateTenantRequest'
      responses:
        '201':
          description: Tenant created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Tenant'

  /admin/service-accounts:
    get:
      tags: [Service Accounts]
      summary: List service accounts
      responses:
        '200':
          description: Service account list
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ServiceAccount'
    post:
      tags: [Service Accounts]
      summary: Create service account
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateServiceAccountRequest'
      responses:
        '201':
          description: Service account created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ServiceAccount'

  /admin/service-accounts/{service_account_id}:
    get:
      tags: [Service Accounts]
      summary: Get service account detail
      parameters:
        - in: path
          name: service_account_id
          required: true
          schema: { type: string, format: uuid }
      responses:
        '200':
          description: Service account detail
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ServiceAccount'

  /admin/service-accounts/{service_account_id}/keys:
    get:
      tags: [Keys]
      summary: List keys for a service account
      parameters:
        - in: path
          name: service_account_id
          required: true
          schema: { type: string, format: uuid }
      responses:
        '200':
          description: Key list
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ServiceAccountKey'
    post:
      tags: [Keys]
      summary: Register a public key for a service account
      parameters:
        - in: path
          name: service_account_id
          required: true
          schema: { type: string, format: uuid }
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/RegisterPublicKeyRequest'
      responses:
        '201':
          description: Key registered
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ServiceAccountKey'

  /admin/service-accounts/{service_account_id}/keys/{key_id}/revoke:
    post:
      tags: [Keys]
      summary: Revoke a key
      parameters:
        - in: path
          name: service_account_id
          required: true
          schema: { type: string, format: uuid }
        - in: path
          name: key_id
          required: true
          schema: { type: string }
      responses:
        '200':
          description: Key revoked

  /admin/policies:
    get:
      tags: [Policies]
      summary: List policies
      responses:
        '200':
          description: Policy list
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Policy'
    post:
      tags: [Policies]
      summary: Create policy
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreatePolicyRequest'
      responses:
        '201':
          description: Policy created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Policy'

  /admin/routes:
    get:
      tags: [Routes]
      summary: List provider routes
      responses:
        '200':
          description: Provider route list
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ProviderRoute'
    post:
      tags: [Routes]
      summary: Create provider route
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateProviderRouteRequest'
      responses:
        '201':
          description: Provider route created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ProviderRoute'

  /admin/usage:
    get:
      tags: [Usage]
      summary: Query usage events
      parameters:
        - in: query
          name: tenant_id
          schema: { type: string, format: uuid }
        - in: query
          name: service_account_id
          schema: { type: string, format: uuid }
        - in: query
          name: from
          schema: { type: string, format: date-time }
        - in: query
          name: to
          schema: { type: string, format: date-time }
      responses:
        '200':
          description: Usage events
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/UsageEvent'

  /admin/audit:
    get:
      tags: [Audit]
      summary: Query audit events
      parameters:
        - in: query
          name: tenant_id
          schema: { type: string, format: uuid }
        - in: query
          name: action
          schema: { type: string }
      responses:
        '200':
          description: Audit events
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/AuditEvent'
```

---

# 11. Future Schema Extensions

## V2
- `provider_attempt_events`
- `trace_spans`
- `idempotency_keys`
- `admin_users`
- `admin_roles`
- `provider_configs`
- `cached_responses`

## V3
- `mtls_clients`
- `anomaly_events`
- `billing_invoices`
- `budget_alerts`
- `approval_workflows`
