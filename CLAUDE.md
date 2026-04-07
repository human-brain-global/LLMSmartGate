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

## Reference Documents (`docs/`)
| Doc ID | File | Purpose |
|--------|------|---------|
| PRD-001 | `docs/PRD-001_prd_detail.md` | Detailed product requirements, user stories, acceptance criteria |
| PRD-000 | `docs/PRD-000_prd_original.md` | Original PRD (superseded by PRD-001) |
| ARC-001 | `docs/ARC-001_architecture_technology_standards.md` | Architecture decisions, tech stack, standards |
| HLD-001 | `docs/HLD-001_high_level_design.md` | High-level design: component interactions, data flow |
| LLD-BE-001 | `docs/LLD-BE-001_backend_low_level_design.md` | Backend implementation: traits, structs, algorithms, SQL |
| LLD-FE-001 | `docs/LLD-FE-001_frontend_low_level_design.md` | Frontend implementation: Svelte components, API client |
| LLD-000 | `docs/LLD-000_erd_sql_openapi_original.md` | ERD, SQL schema, OpenAPI spec (original reference) |
| TST-001 | `docs/TST-001_test_cases_checklist.md` | Test cases, acceptance checklists, coverage targets |

**Read the relevant doc before implementing.** PRD-001 is the source of truth for requirements. LLD-BE-001 has Rust-level design details. LLD-000 has the SQL schema reference.

## Rules
- Rust edition 2024, MSRV 1.85. thiserror in lib, anyhow in main/tests only. No `.unwrap()`.
- No aws-lc-sys -- ring as rustls crypto provider.
- Svelte 5 Runes only (`$state`, `$derived`, `$props`). API calls via `+page.server.ts`.
- Conventional Commits: `feat(auth):`, `fix(streaming):`, `chore(deps):`
- Fix bugs: MUST Debug to find the root causes, ONLY propose and implement long-terms solutions 
- Trunk-based dev, squash merge, short-lived branches.
/
## Quality Checklist
- [ ] **Code**: Clean Code standard
- [ ] **Perf**: no blocking I/O, no unnecessary clones, hot paths cached, benchmarked
- [ ] **Security**: fail-safe, no secrets, parameterized SQL, auth on new endpoints, audit emitted
- [ ] **Gateway**: fmt + clippy + tests pass, coverage >= 80% core, no unwrap/unsafe
- [ ] **Admin**: lint + check + tests pass, coverage >= 70%, accessible
- [ ] **SDK**: ruff + mypy + pytest pass, coverage >= 90% signing, py.typed
- [ ] **Merge**: CI green, 1 approval, conventional commit, no unrelated changes
