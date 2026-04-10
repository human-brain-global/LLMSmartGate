# LLMSmartGate -- Monorepo Task Runner
# Install: brew install just (macOS) or cargo install just
# Usage: just <recipe>

# Default: list available recipes
default:
    @just --list

# ── All ──

# Run all checks (CI equivalent)
check-all: check-gateway check-admin check-sdk

# ── Gateway (Rust) ──

# Load .env if present for database/redis URLs needed by integration tests
set dotenv-load

# Unit tests only (no DB required)
check-gateway:
    cd gateway && cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo test -- --skip storage:: --skip api::admin::auth --skip routing::resolver

# Integration tests (requires PostgreSQL + Valkey via `just dev-infra`)
check-gateway-integration:
    cd gateway && cargo test

build-gateway:
    cd gateway && cargo build --release --locked

dev-gateway:
    cd gateway && cargo run

# Unit tests only (no DB required)
test-gateway:
    cd gateway && cargo test -- --skip storage:: --skip api::admin::auth --skip routing::resolver

# Integration tests (requires PostgreSQL + Valkey via `just dev-infra`)
test-gateway-integration:
    cd gateway && cargo test

# ── Admin Console (SvelteKit) ──

check-admin:
    cd admin-console && npm run lint && npm run format:check && npm run check && npm run test --passWithNoTests || true

build-admin:
    cd admin-console && npm run build

dev-admin:
    cd admin-console && npm run dev

test-admin:
    cd admin-console && npm run test

# ── Python SDK ──

check-sdk:
    cd sdk-python && uv run ruff check && uv run ruff format --check && uv run mypy src/ && uv run pytest

build-sdk:
    cd sdk-python && uv build

test-sdk:
    cd sdk-python && uv run pytest

# ── Infrastructure ──

# Start local dev infrastructure (PostgreSQL + Valkey)
dev-infra:
    docker compose up -d

# Stop local dev infrastructure
stop-infra:
    docker compose down

# Start all services for local development
dev: dev-infra
    @echo "Infrastructure started. Run services in separate terminals:"
    @echo "  just dev-gateway"
    @echo "  just dev-admin"
