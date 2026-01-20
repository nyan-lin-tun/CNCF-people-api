# cncf-people-api

Fast, container-ready HTTP service exposing CNCF people data.

This service provides these endpoints:
- `/local/people` — serves an in-memory copy of `assets/people.json` loaded at startup; if missing, a bundled fallback is used. No env vars required.
- `/people` — serves a cached copy refreshed from the canonical upstream source on GitHub using conditional requests.
- `/example` — serves embedded example data from `assets/example.json`.

The goal is to keep responses under 200ms by serving from memory, pre-gzipping payloads, and honoring ETag/If-None-Match for 304s.

Endpoints:
- `GET /healthz` — health check
- `GET /local/people` — returns an in-memory copy of `assets/people.json` (if present) or a bundled fallback
- `GET /people` — returns a cached copy refreshed from upstream in the background
 - `GET /example` — returns embedded example JSON

Schema
- All endpoints return a JSON array of person objects (no top-level wrapper). This matches the upstream `people.json` shape.

## Data Source & Attribution
- This project references and may embed data from the CNCF People repository: https://github.com/cncf/people (source JSON: `people.json`).
- The upstream repository is licensed under the MIT License. Copyright (c) Cloud Native Computing Foundation.
- When embedding or redistributing the JSON, we include attribution and retain the upstream license. See “Third-Party Notices”.

## Ownership & Affiliation
- This project does not own, control, or guarantee the accuracy of the data.
- This project is not affiliated with or endorsed by CNCF; it provides a cached/proxied view for convenience.

## Privacy & Takedown
- The dataset contains publicly provided information (e.g., names, bios, links, emails as formatted in the source).
- We do not add, infer, or enrich personal data; we only mirror/proxy the upstream JSON.
- To correct or remove your information, please submit a PR or request to the upstream source: https://github.com/cncf/people.
- For urgent takedown requests specific to this service, please create the issue. We will remove cached content and rely on upstream for permanent changes.

## Usage Notes
- `/local/people` reads `assets/people.json` by default; if missing, it serves a small bundled sample (may be stale between releases).
- `/people` serves a cached copy refreshed from upstream via ETag/conditional GET; on errors/timeouts, a previously cached version may be served (stale-while-revalidate behavior).
- Treat this API as a convenience cache; the upstream repository is authoritative.

## Data Files
- `assets/people.json` — your local dataset for `/local/people` (drop your real file here). Loaded into memory at startup.
- `assets/example.json` — fun example dataset used by `/example`.

## Run Locally

Prerequisites: Rust (stable, 1.80+ recommended).

### asdf setup (recommended)
- Install asdf: https://asdf-vm.com/
- Add Rust plugin: `asdf plugin add rust`
- Install toolchain from `.tool-versions`: `asdf install`
- Verify: `rustc --version` shows `1.80.0` or newer.

Environment variables (optional):
- `PORT` (default: 9090)
- `LOCAL_PATH` (default: `assets/people.json`). If missing, a small bundled sample is served.
- `REMOTE_URL` (default: CNCF upstream raw URL)
- `REFRESH_INTERVAL` (e.g., `10m`)

Commands:
- `cargo run` — starts the server on `:9090`
- `curl -i http://localhost:9090/local/people`
- `curl -i http://localhost:9090/people`
 - `curl -i http://localhost:9090/example`

## Container

This project is designed for a small, single-binary image. See the included `Dockerfile` for a multi-stage build using a distroless runtime.

Build images with different tags (same image, just different labels):
- `docker build -t cncf-people-api:latest .`
- `docker build -t cncf-people-api:local .`
- `docker build -t cncf-people-api:example .`
- `docker build -t cncf-people-api:people .`

Run examples:
- Example data (embedded):
  - `docker run --rm -p 9090:9090 cncf-people-api:example`
  - Test: `curl -s http://localhost:9090/example`
- Local people from mounted file:
  - `docker run --rm -p 9090:9090 -v "$PWD/assets/people.json":/assets/people.json:ro cncf-people-api:local`
  - Test: `curl -s http://localhost:9090/local/people`
- Remote people from GitHub (cached):
  - `docker run --rm -p 9090:9090 cncf-people-api:people`
  - Test: `curl -s http://localhost:9090/people`

Multi-arch builds (AMD64 + ARM64):
- Enable buildx: `docker buildx create --use --name multi && docker buildx inspect --bootstrap`
- Build and push (example):
  - `docker buildx build --platform linux/amd64,linux/arm64 -t YOUR_REPO/cncf-people-api:latest --push .`
- Build locally for a single arch (no push):
  - `docker buildx build --platform linux/arm64 -t cncf-people-api:arm64 --load .`
  - `docker run --rm -p 9090:9090 cncf-people-api:arm64`

## Third-Party Notices
See THIRD_PARTY_NOTICES.md for licensing and attribution of upstream data.
