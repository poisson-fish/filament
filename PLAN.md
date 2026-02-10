# Project Filament — PLAN.md
Security-first, open-source, self-hostable Discord-like RTC stack (text + voice/video + screen share).
**No federation** (each server owns its own user DB & auth).

## Core Decisions (locked)
- **Backend language:** Rust (security-first)
- **DB:** PostgreSQL (system-of-record)
- **Search:** Tantivy (derived index; rebuildable cache)
- **Auth tokens:** **PASETO** (prefer over JWT)
- **TLS:** **rustls** (direct or via reverse proxy; still rustls-first)
- **SFU:** **LiveKit** (separate service in docker-compose)
- **Rust WebRTC stack:** track **`rtc`** crate for future Rust-native media (webrtc crate final-revision; not baseline)
- **Desktop client:** Tauri + **SolidJS**
- **HTML embedding:** none (markdown rendered to safe UI primitives; no HTML injection path)

---

## Non-Goals (for now)
- Federation
- E2EE for group rooms / SFU media
- “Discord-scale” global sharding / multi-region
- Server-side media transcoding or mixing (client encodes; SFU forwards)

---

## Target Scale (v0)
- A few hundred users per server
- ~25% concurrently in voice/video
- Single VPS common
- SFU is bandwidth-heavy; server itself stays CPU-light

---

# Architecture Overview

## Components
1) **filament-server** (Rust)
- User auth + local user DB
- Guilds/channels/roles/permissions
- Realtime Gateway (WSS WebSocket)
- REST API for CRUD + pagination + search
- Message persistence (Postgres)
- Search indexing/query (Tantivy)
- Media signaling integration: issues LiveKit room tokens, maps channels<->rooms
- Attachments upload/storage

2) **livekit** (SFU)
- Voice/video/screen share, subscription control, simulcast, NAT traversal, TURN options
- Auth via server-issued LiveKit tokens
- Operates as separate service in compose

3) **filament-client-web** (recommended early)
- Fast iteration for UI + protocol
- Uses browser WebRTC to connect to LiveKit

4) **filament-client-desktop** (Tauri + SolidJS)
- Hardened desktop wrapper around the web UI
- Minimal Rust command surface

5) **filament-client-mobile** (later)
- Native shells or Tauri-mobile once stable
- Strong secure-storage + push notifications

---

# Data & Search Model

## Postgres: source-of-truth
- Users, sessions, roles, permissions
- Guilds, channels, memberships
- Messages, attachments metadata
- Audit logs

## Tantivy: derived search index (cache)
- Index message content + filters
- Search returns message IDs → fetch rows from Postgres
- Index is rebuildable from Postgres (no data loss if index removed)

---

# Security Principles (apply to every phase)
- **Strict untrusted-server model:** clients treat server-provided data as hostile.
- **Rust-first:** minimize FFI; avoid C/C++ in core services.
- **Hard limits everywhere:**
  - request body size caps
  - WS frame/message size caps
  - rate limits by IP/user/session/channel
  - attachment size caps + MIME sniffing
- **No HTML injection path:** markdown parsed to safe UI AST; links sanitized.
- **Auth hardening:** Argon2id, token rotation, replay resistance, device/session management.
- **Least privilege:** Tauri commands whitelisted; server endpoints permission-checked.
- **Observability + auditability:** structured logs, metrics, audit tables.
- **Supply-chain posture:** RustSec, deny lists, review gates, SBOM, pinned toolchains.

---

# Security Baseline Defaults (v0)
These are baseline limits. Tighten by endpoint/route as needed, but never loosen without documented justification.

- HTTP request body cap: `1 MiB` default JSON body, explicit per-route override for larger payloads
- Attachment upload cap: `25 MiB` per file (streaming write required)
- Per-user attachment storage quota: configurable, enforced across all user-owned attachments
- WS frame cap: `64 KiB`
- WS message/event cap: `64 KiB` after decode
- WS outbound queue per connection: `256` events max, then drop/close slow consumer
- REST rate limit baseline: `60 req/min/IP` and stricter per-route auth limits
- Gateway event ingress rate: `20 events/10s/connection` with burst cap
- Search query cap: `256` chars, max `50` results, bounded wildcard/fuzzy usage
- LiveKit token TTL: `5 minutes` max, room-scoped and permission-scoped
- Access token TTL: `15 minutes` max; refresh token rotation on every refresh

---

# Rust Crates (Recommended)

## Workspace-wide foundation
- async/runtime: `tokio`
- http: `axum`, `tower`, `tower-http`
- ws/gateway: `axum::extract::ws` (preferred) + `bytes`
- serialization: `serde`, `serde_json`
- time: `time`
- ids: `ulid` (sortable, canonical project default)
- errors: `thiserror` (libs), `anyhow` (apps)
- logging: `tracing`, `tracing-subscriber`
- config: `config` or `figment` (choose one; default `config`)
- concurrency: `dashmap`, `tokio-util`

## DB
- `sqlx` (+ `sqlx-cli`) with Postgres features
- no deadpool (use sqlx pool)

## Auth/Crypto
- password hashing: `argon2` (Argon2id)
- secrets hygiene: `secrecy`
- RNG: `rand`
- tokens: `paseto` (PASETO)
- TLS (if terminating in app): `rustls`, `tokio-rustls`
- constant-time operations when needed: rely on libs; avoid bespoke crypto

## Rate limiting / abuse
- `tower-governor` (rate limits)
- `ipnet` (CIDR/IP policies)

## Uploads / attachments
- multipart: `axum-multipart` (or `multer` if needed)
- mime: `mime`, `infer`
- hashing: `sha2`
- storage abstraction: `object_store` (local + S3-compatible)
- local storage root path must be configurable via environment (default mounted data dir in compose)

## Search
- `tantivy`

## Markdown (no HTML)
- `pulldown-cmark` (parse)
- `linkify` (optional URL detection)
- IMPORTANT: do not render as HTML; render to UI tokens/elements.

## Metrics
- `metrics`, `metrics-exporter-prometheus`

## WebRTC future (Rust-native)
- `rtc` crate (kept behind feature flags for future SFU replacement / testing harness)
- not required for v0 since LiveKit is baseline

---

# GitHub Actions: Security-First CI (Required)

## CI Goals
- Lint + format + tests
- Dependency auditing (RustSec)
- License + banned deps + advisories policy gates
- Supply chain checks + SBOM
- Optional: fuzzing + sanitizers (nightly/cron)

## Recommended tooling
- **RustSec advisories:** `cargo-audit`
- **Policy enforcement (licenses/bans/sources):** `cargo-deny`
- **Dependency review gate:** `actions/dependency-review-action`
- **SBOM:** `anchore/sbom-action` (CycloneDX/SPDX)
- **Pin toolchains:** `dtolnay/rust-toolchain` or `actions-rs/toolchain` + `rust-toolchain.toml`
- **Reproducible checks:** lockfile enforced, minimal version drift
- Optional high-rigor:
  - `cargo vet` (reviewed dependency intake)
  - `osv-scanner` (extra advisory coverage)
  - `cargo fuzz` (cron or manual)

---

# Repo Layout (suggested)
- `crates/`
  - `filament-protocol/` (events, versioning, validation types)
  - `filament-core/` (domain model, permission bits, invariants/newtypes)
  - `filament-db/` (sqlx queries, migrations integration)
  - `filament-search/` (tantivy schema, indexer, query layer)
  - `filament-auth/` (argon2, paseto, sessions, authz helpers)
  - `filament-media/` (LiveKit integration: room mapping, token minting)
  - `filament-uploads/` (attachments storage + validation)
  - `filament-observability/` (metrics/logging helpers)
  - `filament-rtc/` (future: rtc crate experiments; feature-gated)
- `apps/`
  - `filament-server/`
  - `filament-client-web/`
  - `filament-client-desktop/` (Tauri + SolidJS)
- `infra/`
  - `docker-compose.yml`, reverse proxy configs
- `docs/`
  - `ARCH.md`, `PROTOCOL.md`, `SECURITY.md`, `DEPLOY.md`, `THREAT_MODEL.md`

---

# Phase Template
Every phase has:
- **Status:** `NOT STARTED | IN PROGRESS | DONE`
- **Notes:** important implementation details / pitfalls
- **TODOs:** concrete next tasks
- **Security Outlook:** threat-focused checklist for this phase
- **Exit Criteria:** objective gates (tests + security checks) required before moving phases

---

# Phase 0 — Bootstrap & Security Baseline
**Goal:** foundations that prevent rewrites + enforce security discipline.

### Deliverables
- Workspace + crate skeletons
- `rust-toolchain.toml` pinned
- CI with:
  - fmt, clippy, tests
  - cargo-audit, cargo-deny
  - dependency-review-action
  - SBOM generation
- Initial docs: `SECURITY.md`, `THREAT_MODEL.md`, `PROTOCOL.md` skeleton
- Dev compose: Postgres + filament-server + LiveKit

### Status
- DONE

### Notes
- Decide ID type globally: default **ULID**.
- Decide session strategy early: opaque refresh tokens in DB + short-lived PASETO access token.
- Enforce body limits globally in axum; don’t rely on per-handler.
- 2026-02-10: CI/security baseline implemented (`fmt`, `clippy`, `test`, `cargo audit`, `cargo deny`, dependency review, SBOM workflow).
- 2026-02-10: `filament-server` baseline added with global `DefaultBodyLimit`, request timeout layer, per-IP baseline rate limiting (`tower-governor`), JSON tracing, and request ID propagation.
- 2026-02-10: Added protocol + threat model docs and strict protocol envelope parsing (`{ v, t, d }`) in `filament-protocol`, including version and size checks.
- 2026-02-10: Added dev compose baseline (`infra/docker-compose.yml`) for Postgres + LiveKit + filament-server.

### TODOs
- Phase 1 start gate: implement auth/session persistence with PASETO + refresh rotation and account-enumeration protections.

### Exit Criteria
- CI enforces `fmt`, `clippy`, `test`, `cargo audit`, `cargo deny`, dependency review, SBOM.
- Global body limits, request timeouts, and baseline rate limits are implemented and integration-tested.
- Threat model and protocol docs include concrete abuse cases and compatibility constraints.

### Security Outlook
- Supply chain gates in place before feature work.
- “Untrusted server” mindset documented for clients.

---

# Phase 1 — Auth + Guilds/Channels + Basic Realtime Text
**Goal:** functional Discord-like text.

### Scope
- Register/login/logout
- Session mgmt: PASETO access + refresh in DB
- Guilds + text channels
- Permissions v0 (minimal role/permission checks for moderation-critical actions)
- WS gateway: connect/auth/subscribe/send/receive
- Postgres persistence + history pagination

### Status
- DONE

### Notes
- Prefer **newtypes + TryFrom** for usernames/channel names to ensure invariants.
- WS: enforce max frame size, per-connection quotas, and backpressure (bounded channels).
- Prevent fanout amplification: cap channel subscriber count or apply slow-consumer handling.
- 2026-02-10: Added domain newtypes + invariants in `filament-core` (`Username`, `GuildName`, `ChannelName`, `UserId`) and centralized baseline permission checks (`Role`/`Permission`).
- 2026-02-10: Implemented auth/session HTTP flows in `filament-server` (`/auth/register`, `/auth/login`, `/auth/refresh`, `/auth/logout`, `/auth/me`) with Argon2id password hashing, PASETO v4 local access tokens, refresh-token hashing, refresh rotation, and refresh replay detection.
- 2026-02-10: Added account-enumeration resistance controls (consistent login failure responses + dummy hash verification path) and lockout/backoff guardrails for repeated failed login attempts.
- 2026-02-10: Security/tooling checks run locally for this increment: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo audit`, `cargo deny check --config cargo-deny.toml`.
- 2026-02-10: Added Phase 1 realtime text primitives in `filament-server`: auth-gated guild/channel/message routes (`/guilds`, `/guilds/{guild_id}/channels`, `/guilds/{guild_id}/channels/{channel_id}/messages`) with permission checks and bounded history pagination (`limit` default 20, max 100, optional `before` cursor).
- 2026-02-10: Added `/gateway/ws` with versioned envelope parsing (`{ v, t, d }`), strict gateway event-size enforcement, per-connection ingress event rate controls, bounded outbound queue, and slow-consumer close signaling.
- 2026-02-10: Added per-route auth endpoint rate limits (`register`, `login`, `refresh`) and structured auth audit logs for register/login/lockout/refresh/logout outcomes.
- 2026-02-10: Added/expanded tests for remaining Phase 1 behavior: auth route-specific rate limits, history pagination over persisted channel messages, gateway subscriber broadcast flow, and slow-consumer handling. Full local gates passed (`fmt`, `clippy -D warnings`, workspace tests).
- 2026-02-10: Added a live network gateway integration harness (`apps/filament-server/tests/gateway_network_flow.rs`) that boots a TCP listener, performs websocket auth handshake, validates `ready` + `subscribed` events, and verifies end-to-end `message_create` broadcast flow over a real socket.
- 2026-02-10: Re-ran local security/tooling gates after the network gateway harness addition: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo audit`, `cargo deny check --config cargo-deny.toml`.
- 2026-02-10: Added optional Postgres-backed persistence path for Phase 1 auth/session/guild/channel/message data in `filament-server` using `sqlx` (users, sessions, replay-token hashes, guilds/members, channels, messages) with startup-safe lazy schema initialization and preserved refresh rotation/replay detection semantics. In-memory fallback remains for hermetic tests.
- 2026-02-10: Re-ran local quality/security gates after Postgres persistence addition and dependency tightening (`sqlx` postgres-only features): `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo audit`, `cargo deny check --config cargo-deny.toml`.
- 2026-02-10: Added live Postgres integration coverage in `apps/filament-server/tests/postgres_phase1_flow.rs` for register/login/me/refresh/logout with replay detection, account-enumeration response consistency, guild/channel creation, and persisted message pagination.
- 2026-02-10: CI now provisions Postgres for the Rust test job and exports `FILAMENT_TEST_DATABASE_URL`, ensuring Postgres-backed Phase 1 integration coverage runs on every push/PR.
- 2026-02-10: Cutover policy finalized and implemented: runtime `filament-server` now requires `FILAMENT_DATABASE_URL`; in-memory persistence is test-only and documented in `docs/SECURITY.md`.

### TODOs
- Phase 3 start gate: complete remaining Phase 2 hardening tasks (streaming attachment writes and compose-backed attachment root persistence), then begin Tantivy indexing/query integration.

### Exit Criteria
- Unit tests cover newtype invariants, token mint/verify paths, and permission checks.
- Integration tests cover auth register/login/refresh/logout/me flow including refresh rotation + replay detection and account-enumeration response consistency.
- Integration tests cover gateway message-flow behavior (subscribe + message broadcast), pagination against stored history, slow-consumer handling, and network-level websocket handshake/message flow against a live server instance.
- Postgres-backed integration coverage for auth/session/guild/channel/message flows is wired into CI with a live Postgres service.
- Non-DB fallback cutover policy is documented and enforced for runtime (`FILAMENT_DATABASE_URL` required).

### Security Outlook
- Argon2id parameters set for server-class machines.
- Lock down account enumeration responses.
- Audit logging for auth events (login, refresh, password change).
- Refresh tokens are stored hashed server-side and rotated on refresh with replay detection.
- Gateway abuse controls enforced: strict event-size limit, ingress rate limiting, bounded per-connection outbound queue, and slow-consumer disconnect signaling.

---

# Phase 2 — Attachments + Markdown Rendering (No HTML) + Moderation Basics
**Goal:** “real chat app” quality, safe content.

### Scope
- Attachments upload with limits, MIME sniffing (`infer`), hashing (`sha2`)
- Storage via `object_store` (local by default) with configurable local root path
- Markdown parsing (`pulldown-cmark`) to UI tokens (no HTML)
- Message edit/delete w/ permissions
- Basic moderation: ban/kick, delete messages
- Attachment delete flow for users to reclaim quota

### Status
- IN PROGRESS

### Notes
- Never trust client `Content-Type`; always sniff.
- Implement per-user total storage quota across all user attachments (configurable).
- Local attachment root path is runtime-configured (env), and compose mounts persistent storage at that path.
- Markdown: implement allowlist of features; links sanitized.
- 2026-02-10: Added safe markdown tokenization in `filament-core` using `pulldown-cmark` to a strict UI token model (`MarkdownToken`) with explicit link-scheme filtering (`http`, `https`, `mailto`) and HTML event stripping.
- 2026-02-10: Added Phase 2 message mutation + moderation endpoints in `filament-server`: message edit/delete (`PATCH|DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}`) and moderation routes (`POST /guilds/{guild_id}/members/{user_id}/kick|ban`) with centralized permission checks and audit log writes.
- 2026-02-10: Added attachment storage flow in `filament-server` backed by `object_store` local filesystem root with runtime configuration (`FILAMENT_ATTACHMENT_ROOT`), MIME sniffing (`infer`) with `Content-Type` mismatch rejection, SHA-256 hashing, per-user quota enforcement, auth-gated download, and deterministic quota reclamation on delete.
- 2026-02-10: Added Postgres schema additions for Phase 2 (`attachments`, `guild_bans`, `audit_logs`) with in-memory test fallback parity.
- 2026-02-10: Updated `infra/docker-compose.yml` with `FILAMENT_ATTACHMENT_ROOT` and a persistent `filament-attachments` volume mount for local attachment durability.
- 2026-02-10: Added integration coverage in `apps/filament-server/tests/phase2_attachments_and_markdown.rs` for MIME mismatch rejection, auth-gated download, quota enforcement + quota reclamation after delete, message edit/delete markdown token safety, and moderation route behavior.
- 2026-02-10: Re-ran local quality/security gates for this increment: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo audit`, `cargo deny check --config cargo-deny.toml`.

### TODOs
- Virus scanning optional hook (clamd integration as optional service).
- Replace buffered upload body handling with streaming attachment writes to satisfy upload DoS hardening baseline.
- Document attachment-root persistence and backup guidance in deploy docs (`DEPLOY.md`).

### Exit Criteria
- Integration tests cover upload caps, MIME sniffing mismatch rejection, and auth-gated download.
- Integration tests cover per-user quota enforcement and quota reclamation after attachment deletion.
- Unit tests cover markdown allowlist behavior and link scheme filtering.
- Moderation endpoints enforce permissions and emit audit records.

### Security Outlook
- Upload DoS mitigations: streaming writes + hard size caps.
- Path traversal protections (no user-controlled paths).
- Link handling: disable `file://`, internal schemes.
- Storage exhaustion controls: per-user quotas enforced server-side, with deterministic quota release on delete.

---

# Phase 3 — Tantivy Search
**Goal:** fast search without relying on Postgres FTS.

### Scope
- Tantivy schema + index writer
- Async indexing pipeline (queue) on message commit
- Search API returns message IDs → Postgres fetch

### Status
- NOT STARTED

### Notes
- Treat index as cache: rebuild job from Postgres is mandatory.
- Ensure deletes/edits update index reliably (tombstones or reindex doc).

### TODOs
- Add index reconciliation job (detect missing docs).
- Add per-guild query caps + timeouts.
- Add indexing idempotency tests for edit/delete/reindex flows.

### Exit Criteria
- Integration tests verify search returns IDs, Postgres hydration path, and consistency after edits/deletes.
- Rebuild and reconciliation jobs are documented and tested on seeded data.
- Query-abuse guards (length/complexity/timeout/result cap) are validated in tests.

### Security Outlook
- Query DoS: cap query length, wildcard usage, result limits.
- Avoid user-controlled analyzers; fixed schema only.

---

# Phase 4 — Roles/Permissions v1 + Presence + Reactions + Audit
**Goal:** Discord-like governance + real-time UX.

### Scope
- Roles + permission bits + channel overrides
- Presence via gateway connections
- Reactions
- Audit log table populated for sensitive actions

### Status
- NOT STARTED

### Notes
- Permission checks must be centralized in `filament-core`.
- Presence should not leak private guild membership cross-guild.

### TODOs
- Add “permission snapshot” caching w/ invalidation.
- Add mod tooling endpoints.

### Exit Criteria
- Permission engine has exhaustive unit tests for role hierarchy and channel overrides.
- Integration tests verify privilege boundaries for role mutation/moderation endpoints.
- Presence events are privacy-checked to prevent cross-guild membership leaks.

### Security Outlook
- Privilege escalation review: role mutation endpoints are high risk.
- Ensure least privilege defaults.

---

# Phase 5 — LiveKit Integration (Voice First)
**Goal:** stable voice channels with minimal server CPU.

### Scope
- `filament-media` crate: LiveKit room mapping + token minting
- Client join voice channel → request token → connect to LiveKit
- Speaking indicators (optional)

### Status
- NOT STARTED

### Notes
- Filament server is the policy engine:
  - who can join which room
  - who can publish audio/video
  - who can subscribe
- Tokens must be short-lived and scoped (room + permissions).

### TODOs
- Implement LiveKit key management + rotation plan.
- Document firewall/ports; optional TURN config.
- Add token mint rate limits and audit logging for media token issuance.

### Exit Criteria
- Integration tests verify room-scoped permission-scoped token issuance and denied-path behavior.
- Replay resistance is validated (short TTL + rotation/session checks as designed).
- Operational docs include key rotation and incident response steps.

### Security Outlook
- Prevent token replay: short TTL + optional session binding.
- Enforce publish permissions to block “forced stream injection.”

---

# Phase 6 — Video + Screen Share + Opt-in Streams (LiveKit)
**Goal:** Discord-like opt-in viewing.

### Scope
- Video publish & subscribe
- Screen share tracks
- Opt-in stream subscription UI and server-side policy enforcement

### Status
- NOT STARTED

### Notes
- Opt-in should be enforced in **policy**, not just UI.
- Cap max subscribed streams by default to prevent client DoS.

### TODOs
- Add “stream roles” (who can broadcast).
- Add bandwidth presets and max resolution defaults.

### Exit Criteria
- Integration tests verify unauthorized publish/subscribe attempts are rejected.
- Limits on concurrent subscriptions and publish churn are enforced and tested.
- Server policy remains authoritative regardless of client UI state.

### Security Outlook
- Protect against “stream spam”: rate limit publish/unpublish.
- Prevent unauthorized subscriptions (private channels).

---

# Phase 7 — Desktop Client Hardening (Tauri + SolidJS)
**Goal:** shippable desktop app that’s hostile-server resilient.

### Scope
- Tauri CSP locked down
- Minimal command API surface
- No remote navigation, no remote script execution
- Secure storage for tokens
- Signed updates

### Status
- NOT STARTED

### Notes
- Keep Rust side tiny: filesystem access only if essential.
- Validate all IPC payloads like untrusted network input.

### TODOs
- Define secure token storage strategy per OS.
- Add crash-safe logging (no secrets).
- Add web-client hardening checklist (CSP, allowed URL schemes, no dynamic script execution).

### Exit Criteria
- Tauri command surface is minimized and each command has input validation tests.
- Desktop and web clients pass CSP/navigation hardening checks.
- Token storage strategy is implemented and documented per supported OS.

### Security Outlook
- Explicit threat model: malicious server tries to exploit client via UI.
- Regular dependency auditing for frontend deps too.

---

# Phase 8 — Deployment & Ops (Compose, Backups, Observability)
**Goal:** self-hosting should be trivial and reliable.

### Scope
- Compose: Postgres + filament-server + livekit + reverse proxy
- Backups: Postgres + attachment storage; index rebuild docs
- Metrics: Prometheus endpoint + dashboard templates

### Status
- NOT STARTED

### Notes
- Encourage reverse proxy TLS termination initially (Caddy/Traefik), but keep rustls option.
- Treat Tantivy index as ephemeral; never backup as primary.

### TODOs
- Add `DEPLOY.md` w/ ports, TLS, TURN guidance.
- Add backup/restore scripts.
- Add scheduled restore drill procedure and verification checklist.
- Document attachment storage volume mount + `FILAMENT_ATTACHMENT_ROOT` environment configuration.

### Security Outlook
- Secure defaults: non-root containers, read-only FS where possible, drop caps.
- Secrets via env/secret mounts; no plaintext in compose.

### Exit Criteria
- Compose stack runs with secure defaults (non-root/read-only/drop caps where possible).
- Backup and restore procedures are tested end-to-end on sample data.
- Observability includes dashboards/alerts for auth failures, rate-limit hits, and WS disconnect reasons.

---

# Phase 9 — Mobile (Incremental)
**Goal:** iOS/Android support with secure storage + notifications.

### Scope
- Chat first, then voice, then video/screen share
- Push notifications integration (optional but expected)
- Secure token storage, background voice policies

### Status
- NOT STARTED

### Notes
- Mobile WebRTC uses native stacks; LiveKit SDKs may help.
- Keep protocol compatibility identical across clients.

### TODOs
- Decide mobile framework strategy.
- Add threat model for push + notification metadata.

### Security Outlook
- Token handling on mobile is often the weakest link: harden storage + rotation.
- Privacy: minimize notification content.

---

# Definition of Done (v0)
- Secure local auth + roles/permissions
- Real-time text with persistent history + Tantivy search
- Attachments with validation/limits
- LiveKit-backed voice/video/screen share
- Opt-in streams enforced by server policy
- Desktop client (Tauri + Solid) hardened for malicious server input
- CI security gates + deployable docker-compose
