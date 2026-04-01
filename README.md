# LLMSmartGate

Self-hosted LLM API Gateway providing unified, secure, policy-governed access to multiple LLM providers (OpenAI, Anthropic, Google Gemini, Azure OpenAI, vLLM).

## Features

- **OpenAI-compatible API** -- drop-in replacement, existing client code works without modification
- **Ed25519 request signing** -- asymmetric authentication, no shared secrets
- **Policy engine** -- model access control, token limits, budget enforcement, rate limiting
- **Multi-provider routing** -- alias resolution, priority-based fallback chains, automatic retry
- **SSE streaming relay** -- zero-copy forwarding with client disconnect detection
- **Usage metering** -- token counting, cost estimation, append-only audit trail
- **Admin console** -- browser-based management UI for tenants, policies, routes, and usage

## Architecture

```
                    +-------------------+
                    |   Admin Console   |  (SvelteKit)
                    +--------+----------+
                             |
[Client SDK] --HTTPS--> [Gateway Service] --HTTPS--> [LLM Providers]
  (Python)               (Rust / Axum)                OpenAI, Anthropic,
  Ed25519 signed         Auth -> Policy -> Route      Gemini, Azure, vLLM
                         -> Proxy -> Meter -> Audit
                             |
                    +--------+----------+
                    | PostgreSQL|Valkey  |
                    +-------------------+
```

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable, edition 2024)
- [Node.js](https://nodejs.org/) >= 22
- [uv](https://docs.astral.sh/uv/) (Python package manager)
- [Docker](https://www.docker.com/) (for PostgreSQL + Valkey)
- [just](https://github.com/casey/just) (task runner)

### Setup

```bash
# Clone and enter the repo
git clone <repo-url> && cd llmsmartgate

# Start infrastructure (PostgreSQL + Valkey)
just dev-infra

# Copy environment config
cp .env.example .env

# Run gateway
just dev-gateway

# Run admin console (separate terminal)
just dev-admin
```

### Common Commands

```bash
just                 # List all available recipes
just check-all       # Run all checks (gateway + admin + sdk)
just check-gateway   # Rust: fmt + clippy + tests
just check-admin     # SvelteKit: lint + format + check + tests
just check-sdk       # Python: ruff + mypy + pytest
just dev-infra       # Start PostgreSQL + Valkey
just stop-infra      # Stop infrastructure
```

## Project Structure

```
gateway/            Rust (Axum 0.8, Tokio, SQLx) -- data plane + admin API
admin-console/      SvelteKit 2 (Svelte 5, TypeScript, Tailwind v4) -- admin UI
sdk-python/         Python 3.12+ (httpx, pydantic, PyNaCl) -- client SDK
deploy/             Helm charts for Kubernetes deployment
docs/               Architecture documents (PRD, ARC, HLD, LLD, TST)
```

## Documentation

| Document | Description |
|----------|-------------|
| [PRD-001](docs/PRD-001_prd_detail.md) | Product Requirements |
| [ARC-001](docs/ARC-001_architecture_technology_standards.md) | Architecture & Technology Standards |
| [HLD-001](docs/HLD-001_high_level_design.md) | High-Level Design |
| [LLD-BE-001](docs/LLD-BE-001_backend_low_level_design.md) | Backend Low-Level Design |
| [LLD-FE-001](docs/LLD-FE-001_frontend_low_level_design.md) | Frontend Low-Level Design |
| [TST-001](docs/TST-001_test_cases_checklist.md) | Test Cases & Checklist |

## License

MIT OR Apache-2.0
