# Developer Journey

This document captures our plan, decisions, current status, and next steps for cncf-people-api so we can pick up quickly tomorrow.

**Project Goal**
- Fast, reliable API (<200ms) exposing CNCF people data.
- Endpoints: `GET /local/people` (local bundle) and `GET /people` (cached from upstream).
- Serve from memory with ETag/conditional GET and gzip; containerize and add CI/CD.

**Key Decisions**
- Language/Framework: Rust + Axum on Tokio; Reqwest (rustls) for HTTP.
- Response features: ETag, Cache-Control, gzip (tower-http compression-gzip).
- Caching strategy: Background refresh for `/people` with `If-None-Match`; serve stale on errors.
- Data sourcing: Reference upstream `cncf/people` (MIT); attribution in README and THIRD_PARTY_NOTICES.
- Tooling: asdf for tool versions (`rust 1.76.0`).
- Container: multi-stage Dockerfile; distroless runtime, non-root user.
- CI: GitHub Actions for cargo build + Docker build.

**Current Status (2026-01-20)**
- Implemented Axum server with routes:
  - `/healthz` returns OK.
  - `/local/people` reads `people.json` from disk per request with ETag + cache headers.
  - `/people` serves a background-refreshed cache from GitHub raw with conditional GET.
- Added gzip compression layer, env-configurable `PORT`, `LOCAL_PATH`, `REMOTE_URL`, `REFRESH_INTERVAL`.
- Fixed build issues (axum 0.7 server, header type mismatches, tower-http features, humantime dep).
- Dockerfile (distroless) added; `.tool-versions` pinned Rust 1.76.0.
- CI workflow added (cargo build + Docker build).
- Docs updated: README quickstart, attribution, privacy/takedown; THIRD_PARTY_NOTICES created.
- .gitignore extended for editors/OS files/logs; port 9090 set as default.

**Remaining Work (Prioritized)**
- Include local data in binary
  - Switch `/local/people` to `include_bytes!` and precompute ETag.
  - Optional: pre-gzip embedded bytes for zero on-the-fly cost.
- Improve caching performance
  - Pre-gzip remote cached payload as well; set `Vary: Accept-Encoding`.
- CI polish
  - Add `cargo fmt --check` and `cargo clippy -D warnings` jobs.
  - Consider `rust-toolchain.toml` for non-asdf users.
- Observability & Ops
  - Switch logs to JSON format for prod; optionally add Prometheus metrics.
  - Add readiness semantics (e.g., ready when local is loaded; remote can be warming).
- CORS (if called from browsers)
  - Configure allowlist via env; add tower-http CORS layer.
- Delivery
  - Configure Docker push to GHCR/Docker Hub on tags; versioning strategy (SemVer tags).
  - Add minimal Kubernetes manifests (Deployment, Service) if needed.
- Documentation
  - Replace placeholder contact email in README.
  - Add License section for this API (MIT/Apache-2.0) as desired.
- Testing
  - Add integration tests for endpoints; mock remote fetch (e.g., with a local test server).

**Local Dev Notes**
- Run: `cargo run` (use `PORT=9091` if 9090 is busy).
- Test: `curl -i localhost:9090/healthz`, `/local/people`, `/people`.
- Network hiccups to GitHub during initial fetch are non-fatal; `/people` will serve local fallback until cache warms.

**Tomorrow’s First Tasks**
- Implement `include_bytes!` for `/local/people` with precomputed ETag and optional pre-gzip.
- Add `Vary: Accept-Encoding` and pre-gzip path for remote cache.
- Add fmt/clippy checks in CI and update README contact.
- Build and run Docker image locally; decide on registry and push settings.
