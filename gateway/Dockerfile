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
RUN cargo auditable build --release --locked

# Stage 3: Runtime (Chainguard static -- zero-CVE, SBOMs, Sigstore signatures)
FROM cgr.dev/chainguard/static:latest
COPY --from=builder /app/target/release/llmsmartgate /usr/local/bin/
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/llmsmartgate"]
