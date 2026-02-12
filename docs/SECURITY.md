# Filament Security Baseline

## Boundary Limits
- HTTP JSON body default cap: `1 MiB`.
- WebSocket frame cap: `64 KiB`.
- WebSocket decoded event cap: `64 KiB`.
- Baseline REST rate limit: `180 requests/minute/client IP` (override with `FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE`).
- Gateway ingress cap: `20 events/10s/connection` (Phase 1 implementation target).

## Timeouts
- Default request timeout: `10 seconds`.
- Idle/read/write gateway timeouts are mandatory in gateway implementation phases.

## Logging and Correlation
- Structured JSON logs are required.
- Every request includes an `x-request-id` correlation identifier.
- Security-sensitive events (auth, refresh, moderation, rate-limit violations) must be auditable.

## Identity and IDs
- Project-wide identity format is ULID.
- UUID fallback is not used for domain IDs.

## Key Management Policy (PASETO)
- Access token keys are versioned with `kid`.
- Rotation cadence: every `90 days` or sooner after incident indicators.
- Emergency revocation: remove compromised `kid`, reject tokens signed with revoked keys, force refresh/token re-auth.
- Maintain an active key set containing current + previous key during controlled rotation windows.

## Refresh Token Policy
- Refresh tokens are opaque, high-entropy, and stored hashed in Postgres.
- Rotation is required on every refresh.
- Replay detection is mandatory: if an old refresh token is replayed, revoke the session family.

## Persistence Cutover Policy
- Production runtime requires `FILAMENT_DATABASE_URL`; in-memory persistence is not permitted for deployed server processes.
- In-memory persistence remains test-only for hermetic unit/integration coverage where Postgres is intentionally unavailable.

## Upload and Content Safety
- Never trust client-provided `Content-Type`; MIME sniff with `infer`.
- Enforce hard upload caps and streaming writes.
- Enforce configurable per-user attachment storage quotas across all user-owned attachments.
- Attachment storage root path is configured by environment (`FILAMENT_ATTACHMENT_ROOT`) and must point to a non-user-controlled server path.
- Attachment delete operations must reclaim quota deterministically.
- Markdown is transformed into safe UI tokens; no raw HTML rendering.

## LiveKit Voice Token Issuance
- `filament-server` is the policy engine for media room join/publish privileges.
- Voice tokens are room-scoped, permission-scoped, and capped to a maximum `5 minute` TTL.
- Token minting is rate-limited per user/IP/channel and issuance is written to audit logs.
- LiveKit API key and secret are required runtime secrets (`FILAMENT_LIVEKIT_API_KEY`, `FILAMENT_LIVEKIT_API_SECRET`).

## LiveKit Video/Screen Policy
- Publish source permissions are enforced server-side (`microphone`, `camera`, `screen_share`) and filtered from requested sources when issuing tokens.
- Subscribe access is opt-in per token request and enforced server-side by `subscribe_streams` permission checks.
- Video/screen publish churn is rate-limited separately from baseline media token issuance.
- Concurrent subscribe-capable tokens are bounded per user/channel to reduce stream fanout abuse and client DoS risk.
