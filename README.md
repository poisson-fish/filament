# Filament

Filament is a security-first, self-hosted Discord-like platform for realtime chat plus voice/video via LiveKit.

The backend is Rust (`filament-server`) with PostgreSQL as source-of-truth, Tantivy for derived search, and a strict protocol/security posture designed for hostile network input.

## Current Status

Implementation is actively tracked in `PLAN.md`.

- Completed through Phase 8 (server, auth, gateway, attachments, search, roles/moderation, LiveKit integration, desktop hardening, deployment/ops baseline)
- Phase 9 (mobile) is planned

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

Useful commands:

```bash
# View service status
docker compose -f infra/docker-compose.yml ps

# View logs
docker compose -f infra/docker-compose.yml logs -f filament-server

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

