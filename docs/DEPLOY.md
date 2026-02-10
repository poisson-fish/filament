# Deployment Guide (v0)

This document covers deployment, backup/restore, and observability for Filament.

## Compose Topology

`infra/docker-compose.yml` includes:
- `postgres`: source-of-truth database
- `livekit`: SFU for voice/video/screen share
- `filament-server`: API + gateway + search + attachment metadata
- `reverse-proxy`: edge ingress (Caddy) forwarding to `filament-server`

Network model:
- `filament-internal`: internal-only network for service-to-service traffic
- `filament-edge`: edge network for externally exposed ports

Port map (default compose):
- `reverse-proxy`: `8080/tcp` (HTTP ingress to Filament API/gateway)
- `livekit`: `7880/tcp` (signaling), `7881/tcp` (RTC over TCP), `7882/udp` (RTC over UDP)

## Security Defaults in Compose

The compose baseline applies hardening controls where practical:
- non-root runtime for `filament-server` image
- `no-new-privileges` enabled across services
- `cap_drop: [ALL]` on edge/server/media containers
- read-only root filesystem for `filament-server` and `livekit`
- explicit writable mounts only for required data paths (`pg-data`, `filament-attachments`, Caddy state)

Do not remove these defaults without documenting threat impact.

## Required Runtime Environment

Set these variables for `filament-server`:
- `FILAMENT_DATABASE_URL`: required in runtime; points to Postgres
- `FILAMENT_ATTACHMENT_ROOT`: required attachment object storage root
- `FILAMENT_LIVEKIT_API_KEY`: required LiveKit API key for token minting
- `FILAMENT_LIVEKIT_API_SECRET`: required paired LiveKit secret
- `FILAMENT_LIVEKIT_URL`: required signaling URL exposed to clients (`ws://` or `wss://`)
- `FILAMENT_BIND_ADDR`: bind socket for server process (default `0.0.0.0:3000`)

Default compose values:
- `FILAMENT_ATTACHMENT_ROOT=/var/lib/filament/attachments`
- `FILAMENT_LIVEKIT_URL=ws://livekit:7880`
- `FILAMENT_BIND_ADDR=0.0.0.0:3000`

## Attachment Storage Persistence

Attachment binaries are stored under `FILAMENT_ATTACHMENT_ROOT` via `object_store` local backend.
In compose, this path is mounted to a named volume:
- volume: `filament-attachments`
- mount path: `/var/lib/filament/attachments`

Operational requirements:
- Keep `FILAMENT_ATTACHMENT_ROOT` outside any user-controlled writable path.
- Ensure the mount has enough capacity for configured quotas and growth.
- Do not share the same path between unrelated environments (dev/stage/prod).

## TLS and Reverse Proxy

Use TLS at the edge proxy in production.

Baseline requirements:
- redirect HTTP to HTTPS
- HSTS on public hosts
- upstream proxy to `filament-server:3000`
- pass websocket upgrades for `/gateway/ws`
- restrict direct public exposure of `filament-server`

`infra/Caddyfile` is a baseline ingress config. For production, replace `auto_https off` with certificate-backed HTTPS settings.

## LiveKit and TURN Guidance

For internet-facing voice/video quality and NAT traversal:
- keep UDP `7882` reachable where possible
- configure TURN servers for restrictive NATs/firewalls
- prefer TLS (`wss://`) signaling for client connections

LiveKit token policy:
- voice/media token issuance is channel-scoped and permission-scoped
- token TTL is capped at `5 minutes`
- minting is rate-limited and audit logged (`media.token.issue`)

### LiveKit key rotation baseline

1. Generate a new API key/secret in LiveKit.
2. Deploy new `FILAMENT_LIVEKIT_API_KEY` + `FILAMENT_LIVEKIT_API_SECRET` to `filament-server`.
3. Restart `filament-server` and verify token issuance.
4. Revoke old keys in LiveKit.

## Backup and Restore

Back up both:
- Postgres data (`postgres.dump`)
- attachment volume (`attachments.tar.gz`)

Tantivy index is a rebuildable cache and is not backup-primary.

### Scripts

Scripts are provided under `infra/scripts/`:
- `backup.sh [backup_root]`
- `restore.sh <backup_dir>`

Defaults:
- compose file: `infra/docker-compose.yml`
- compose project: `filament`
- artifacts are checksummed with `SHA256SUMS`

Example:

```bash
infra/scripts/backup.sh
infra/scripts/restore.sh backups/20260210T120000Z
```

### Scheduled Restore Drill (required)

Run at least monthly in a staging environment:

1. Take a fresh backup with `infra/scripts/backup.sh`.
2. Restore using `infra/scripts/restore.sh` into isolated staging.
3. Verify `GET /health` returns `200`.
4. Verify auth login and one known attachment download.
5. Rebuild and reconcile search index for each guild:
   - `POST /guilds/{guild_id}/search/rebuild`
   - `POST /guilds/{guild_id}/search/reconcile`
6. Verify new message create/search/edit/delete behavior after rebuild.
7. Record drill outcome and remediation items.

## Observability

Prometheus scrape target:
- `GET /metrics` from `filament-server` (via reverse proxy or internal network)

Key security counters:
- `filament_auth_failures_total{reason=...}`
- `filament_rate_limit_hits_total{surface=...,reason=...}`
- `filament_ws_disconnects_total{reason=...}`

Templates:
- alert rules: `infra/observability/prometheus-alerts.yml`
- dashboard: `infra/observability/grafana-filament-security-dashboard.json`

Alerting minimums:
- auth failure spike
- rate-limit spike
- websocket disconnect spike
