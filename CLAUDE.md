# LLMSmartGate

Self-hosted LLM API Gateway -- unified, secure, policy-governed access to LLM providers.

**PERFORMANCE AND SECURITY ARE FIRST. Evaluate every change through these lenses before all others.**

## Performance: gateway overhead < 10ms p50, < 50ms p95
- All I/O async (Tokio). No blocking on request path. Zero-copy streaming (`bytes::Bytes`).
- Cache: moka (~1us) -> Valkey (~0.5ms) -> PostgreSQL (~2-5ms). Usage persistence is fire-and-forget.
- Benchmark with criterion. No unbounded allocations, clones, or pool growth.

## Security: fail-safe, deny on ambiguous failure
- Defense in depth: TLS + Ed25519 signing + nonce replay + policy + rate limit + audit.
- No secrets in code/configs/logs. Parameterized queries only. Append-only audit trail.
- Dependencies audited (`cargo audit/deny`). Images scanned. Actions pinned to SHAs.

## Structure
```
gateway/          Rust (Axum 0.8, Tokio, SQLx) -- data plane + admin API
admin-console/    SvelteKit 2 (Svelte 5, TypeScript, Tailwind v4) -- admin UI
sdk-python/       Python 3.12+ (httpx, pydantic, PyNaCl) -- client SDK
deploy/           Helm charts    docs/   PRD, ARC, HLD, LLD, TST
```

## Commands (via Justfile)
```bash
just check-all       # Run all checks (gateway + admin + sdk)
just check-gateway   # fmt + clippy + nextest
just check-admin     # lint + format + check + test
just check-sdk       # ruff + mypy + pytest
just dev-infra       # Start PostgreSQL + Valkey
just dev-gateway     # Run gateway locally
just dev-admin       # Run admin console locally
```

## Rules
- Rust edition 2024, MSRV 1.85. thiserror in lib, anyhow in main/tests only. No `.unwrap()`.
- No aws-lc-sys -- ring as rustls crypto provider.
- Svelte 5 Runes only (`$state`, `$derived`, `$props`). API calls via `+page.server.ts`.
- Conventional Commits: `feat(auth):`, `fix(streaming):`, `chore(deps):`
- Trunk-based dev, squash merge, short-lived branches.

## Quality Checklist
- [ ] **Perf**: no blocking I/O, no unnecessary clones, hot paths cached, benchmarked
- [ ] **Security**: fail-safe, no secrets, parameterized SQL, auth on new endpoints, audit emitted
- [ ] **Gateway**: fmt + clippy + tests pass, coverage >= 80% core, no unwrap/unsafe
- [ ] **Admin**: lint + check + tests pass, coverage >= 70%, accessible
- [ ] **SDK**: ruff + mypy + pytest pass, coverage >= 90% signing, py.typed
- [ ] **Merge**: CI green, 1 approval, conventional commit, no unrelated changes
