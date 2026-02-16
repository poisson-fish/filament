# Filament

Filament is a security-first, self-hosted Discord-like platform for realtime chat, voice, video, and screen sharing.

It is built around a hardened Rust backend (`filament-server`), PostgreSQL as source-of-truth, Tantivy as a rebuildable search index, and LiveKit as the SFU for media transport.

## At a Glance

- Security-first architecture with strict request/message limits and rate limiting
- Realtime text over WebSocket gateway plus REST API for CRUD/search/admin flows
- Voice/video/screen share via server-issued, scoped LiveKit tokens
- Self-hostable with Docker Compose baseline
- Web and desktop clients (mobile planned)

## Current Status

Implementation is actively tracked in `PLAN.md`.

- Completed through Phase 8 (server, auth, gateway, attachments, search, roles/moderation, LiveKit integration, desktop hardening, deployment/ops baseline)
- Phase 9 (mobile) is planned

## Architecture

```mermaid
flowchart LR
  web[Web Client<br/>SolidJS]
  desktop[Desktop Client<br/>Tauri + SolidJS]
  caddy[Reverse Proxy<br/>Caddy]
  server[filament-server<br/>Rust + Axum]
  postgres[(PostgreSQL<br/>source of truth)]
  tantivy[(Tantivy<br/>derived index/cache)]
  attachments[(Attachment Storage<br/>object_store/local volume)]
  livekit[LiveKit SFU<br/>voice/video/screen]

  web --> caddy
  desktop --> caddy
  caddy --> server
  server --> postgres
  server --> tantivy
  server --> attachments
  server -->|token issuance + policy enforcement| livekit
  web --> livekit
  desktop --> livekit
```

Design principles:

- Untrusted-input model at every network boundary (client and server)
- Domain invariants and validated DTO-to-domain conversion
- Bounded queues, payload caps, rate limits, and timeouts by default
- Search index treated as cache, never as sole source of truth

## Core Features

- Authentication and sessions:
  - Argon2id password hashing
  - PASETO access tokens + rotating refresh tokens
  - Anti-enumeration friendly auth behavior
- Realtime messaging:
  - Versioned gateway envelope (`{ v, t, d }`)
  - Guilds, channels, message history, pagination, reactions
- Content safety:
  - Markdown to safe token model (no HTML embed/render path)
  - Link sanitization and strict parsing
- Attachments:
  - Size limits, MIME sniffing, quota enforcement, secure storage paths
- Moderation and authorization:
  - Roles/permissions model, membership controls, audit-oriented operations
- Search:
  - Tantivy-backed query with bounded complexity and result caps
  - Rebuild/reconcile flows from Postgres
- Media:
  - Channel kinds: `text` and `voice` (`voice` is the RTC-capable channel kind)
  - LiveKit integration for voice/video/screen share
  - Short-lived, scoped, permission-limited media tokens
  - Explicit RTC UX states and troubleshooting for reconnect, permission denial, and token/session expiry

## Technology Stack

| Area | Technologies |
|---|---|
| Server | Rust, Tokio, Axum, Tower |
| Auth | Argon2id, PASETO, refresh-token rotation |
| Database | PostgreSQL + `sqlx` |
| Search | Tantivy (derived index) |
| Media | LiveKit SFU |
| Clients | SolidJS web, Tauri + SolidJS desktop |
| Infra | Docker Compose, Caddy |
| Security/Quality | `cargo audit`, `cargo deny`, clippy, tests, SBOM |

## Project Structure

- `apps/filament-server`: Rust API + gateway + auth + search + attachment metadata
- `apps/filament-client-web`: SolidJS web client
- `apps/filament-client-desktop`: Tauri + SolidJS desktop client
- `crates/`: shared Rust crates (`filament-core`, `filament-protocol`, etc.)
- `infra/`: Docker Compose, ingress config, backup/restore scripts, observability assets
- `docs/`: API, protocol, security, threat model, deployment guides

## Documentation

- Plan and roadmap: [`PLAN.md`](PLAN.md)
- API reference: [`docs/API.md`](docs/API.md)
- Gateway protocol: [`docs/PROTOCOL.md`](docs/PROTOCOL.md)
- Security model and controls: [`docs/SECURITY.md`](docs/SECURITY.md)
- Threat model: [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md)
- Client hardening: [`docs/CLIENT_SECURITY.md`](docs/CLIENT_SECURITY.md)
- Deployment and operations: [`docs/DEPLOY.md`](docs/DEPLOY.md)

## Run with Docker Compose

Prerequisites:

- Docker Engine (or Docker Desktop)
- Docker Compose v2 (`docker compose`)

From the repository root:

```bash
docker compose -f infra/docker-compose.yml up -d --build
```

This starts:

- `postgres`
- `livekit`
- `filament-server`
- `reverse-proxy` (Caddy)

Default local endpoints:

- Filament API/Gateway (via proxy): `http://localhost:8080`
- Health check: `http://localhost:8080/health`
- LiveKit signaling: `ws://localhost:7880`
- Metrics endpoint: `http://localhost:8080/metrics`

Useful commands:

```bash
# View service status
docker compose -f infra/docker-compose.yml ps

# View logs
docker compose -f infra/docker-compose.yml logs -f filament-server

# Check health quickly
curl -fsS http://localhost:8080/health

# Stop services
docker compose -f infra/docker-compose.yml down

# Stop and remove volumes (destructive: deletes local data)
docker compose -f infra/docker-compose.yml down -v
```

## Local Quality Checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```
