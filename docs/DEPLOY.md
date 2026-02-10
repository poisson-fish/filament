# Deployment Guide (v0)

This document covers baseline deployment and storage durability requirements for Filament.

## Services

`infra/docker-compose.yml` includes:
- `postgres`: source-of-truth database
- `livekit`: SFU for voice/video/screen share
- `filament-server`: API + gateway + attachment metadata

## Required Runtime Environment

Set these variables for `filament-server`:
- `FILAMENT_DATABASE_URL`: required in runtime; points to Postgres
- `FILAMENT_ATTACHMENT_ROOT`: required for attachment object storage root
- `FILAMENT_LIVEKIT_API_KEY`: required in runtime; LiveKit API key for server-minted tokens
- `FILAMENT_LIVEKIT_API_SECRET`: required in runtime; paired secret for token signing
- `FILAMENT_LIVEKIT_URL`: required for clients; `ws://` or `wss://` LiveKit signaling URL

Default compose value:
- `FILAMENT_ATTACHMENT_ROOT=/var/lib/filament/attachments`
- `FILAMENT_LIVEKIT_URL=ws://livekit:7880`

## Attachment Storage Persistence

Attachment binaries are stored under `FILAMENT_ATTACHMENT_ROOT` via `object_store` local backend.
In compose, this path is mounted to a named volume:
- volume: `filament-attachments`
- mount path: `/var/lib/filament/attachments`

Operational requirements:
- Keep `FILAMENT_ATTACHMENT_ROOT` outside any user-controlled writable path.
- Ensure the mount has enough capacity for configured quotas and growth.
- Do not share the same path between unrelated environments (dev/stage/prod).

## Backup and Restore

Back up both:
- Postgres data (system of record for metadata and auth/session state)
- Attachment volume (`FILAMENT_ATTACHMENT_ROOT`)

Tantivy index is rebuildable cache and should not be treated as primary backup data.

### Backup cadence (baseline)

- Postgres: daily full backup + frequent WAL/archive policy
- Attachments: daily snapshot or rsync-style incremental copy

### Restore validation checklist

1. Restore Postgres to a clean environment.
2. Restore attachment volume to the configured `FILAMENT_ATTACHMENT_ROOT`.
3. Start compose stack and verify `/health`.
4. Verify auth login and attachment download for a known record.
5. Rebuild and reconcile search index for each guild:
   - `POST /guilds/{guild_id}/search/rebuild`
   - `POST /guilds/{guild_id}/search/reconcile`
6. Verify new upload and deletion both succeed (quota accounting remains correct).

## Network/TLS Notes

- Prefer reverse-proxy TLS termination with modern ciphers and HTTP->HTTPS redirect.
- Keep `filament-server` and `livekit` bound to private network interfaces when possible.
- Expose only required ports at the edge.

## LiveKit Token Policy

- Voice token issuance is channel-scoped and signed by `filament-server`.
- Token TTL is capped at `5 minutes` (`MAX_LIVEKIT_TOKEN_TTL_SECS`).
- Minting is rate-limited per user/IP/channel and every issuance is audit logged (`media.token.issue`).

### LiveKit key rotation baseline

1. Generate a new API key/secret in LiveKit and deploy it to `filament-server` secrets.
2. Restart `filament-server` with new `FILAMENT_LIVEKIT_API_KEY` + `FILAMENT_LIVEKIT_API_SECRET`.
3. Revoke old keys in LiveKit once new issuance has been validated.
4. During incident response, rotate immediately and invalidate old keys before service restore.

## Security Defaults

- Do not run containers as root when image/runtime constraints allow non-root.
- Minimize writable filesystem surfaces beyond data mounts.
- Keep secrets in environment/secret mounts; never commit secrets to the repo.
