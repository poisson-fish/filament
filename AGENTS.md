# AGENTS.md — Project Filament
Instructions for autonomous/codegen agents (LLMs) working in this repo.

This project is **security-first**. Treat **all network input as hostile**, including data from a Filament server (clients must be hardened against malicious servers).

---

## 0) Prime Directives (Read First)
1. **Do not weaken security posture** to ship faster.
2. **No “stringly-typed” domain logic.** Use domain newtypes + invariant constructors.
3. **No HTML embedding/rendering in chat.** Markdown must render to a safe UI token model.
4. **Hard limits everywhere**: request sizes, WS message sizes, uploads, rates, fanout.
5. **Never implement crypto by hand.** Use vetted crates; avoid bespoke schemes.
6. **Prefer minimal feature surface** over “feature-complete” if unclear.
7. **All changes must include tests** (unit/integration) and pass CI security gates.

---

## 1) Repo Overview
Filament is a self-hosted Discord-like app:
- Rust backend (`filament-server`) with:
  - Auth (PASETO + refresh tokens stored in Postgres)
  - Realtime gateway (WSS WebSocket)
  - REST CRUD + pagination
  - Postgres persistence
  - Tantivy search (derived index)
  - LiveKit integration (SFU in docker-compose)
- Clients:
  - Web client (fast iteration)
  - Desktop client (Tauri + SolidJS) hardened
  - Mobile later

**Federation is out-of-scope.** Each server maintains its own user DB/logins.

---

## 2) What Agents May and May Not Do

### Allowed
- Implement features as described in `PLAN.md` phases
- Add tests, docs, CI workflows, and security checks
- Refactor for correctness and safety
- Add strict validation and caps (even if not explicitly requested)

### Forbidden
- Adding HTML rendering paths for chat content
- Disabling security checks, rate limits, or body limits for “dev convenience”
- Adding unsafe Rust without explicit owner approval
- Introducing Node/Electron runtime privileges in the desktop app
- Integrating unlicensed / unclear-license dependencies
- Adding federation or E2EE unless a phase explicitly adds it

---

## 3) Code Style and Conventions

### Rust
- No `unsafe` unless explicitly approved and justified in docs.
- Prefer:
  - `thiserror` for library errors
  - `anyhow` at binary boundaries
- Use `tracing` for logs; no `println!` in production code.
- Enforce clippy pedantic rules selectively; do not suppress warnings broadly.
- Use `time` crate for timestamps.
- IDs: use the project’s chosen ID type (default ULID unless repo says otherwise).

### Protocol
- All gateway events are versioned:
  - `{ v, t, d }`
- Add new event types with backward-compatible parsing.
- Reject unknown/oversized payloads early.

### Testing
- Unit tests for:
  - newtype invariants
  - permission checks
  - auth token mint/verify
- Integration tests for:
  - REST endpoints
  - gateway handshake + chat message flow
  - search indexing + query correctness

---

## 4) Security Checklist (Apply to Every PR)
**Network boundaries**
- Cap HTTP body sizes globally.
- Cap WS frame sizes and message sizes.
- Implement timeouts (request + idle).
- Add rate limits (IP + user + per-route).

**Parsing**
- Validate all input at the boundary:
  - deserialize into DTO structs
  - convert into domain types (`TryFrom`) that enforce invariants
- Never trust `Content-Type` for uploads. MIME sniff using `infer`.

**Auth**
- Passwords: `argon2` Argon2id with sane parameters.
- Tokens: PASETO access tokens short-lived.
- Refresh tokens: stored hashed in DB; rotation supported.
- Protect against account enumeration (consistent auth errors).

**Storage**
- Path traversal protections: never use user strings in paths.
- Attachment quotas and size limits.
- Search index treated as cache; never sole source of truth.

**DoS / Abuse**
- Bounded queues for fanout.
- Slow consumer handling (drop/close after thresholds).
- Search query caps (length, complexity, result limit).
- Upload caps and streaming writes.

**Client Hardening**
- Treat server data as malicious:
  - no remote scripts
  - strict CSP
  - sanitize/validate all IPC payloads in Tauri
- Do not add privileged Tauri commands unless strictly necessary.

---

## 5) Dependency and Supply Chain Rules
- All Rust dependencies must be:
  - permissive license (MIT/Apache/BSD/ISC)
  - actively maintained or justified
- CI includes:
  - `cargo audit` (RustSec advisories)
  - `cargo deny` (licenses/bans/sources)
  - GitHub dependency review gate
  - SBOM generation
- Don’t add dependencies that fail `cargo deny` without updating policy and justification.

---

## 6) Component-Specific Guidance

### 6.1 filament-server (Rust)
**Preferred crates**
- Runtime: `tokio`
- HTTP/WebSocket: `axum`, `tower`, `tower-http`
- DB: `sqlx` (pooling via sqlx)
- Auth: `argon2`, `paseto`, `secrecy`
- Rate limit: `tower-governor`
- Search: `tantivy`

**Gateway**
- Must enforce:
  - max event size
  - per-connection outbound queue bounds
  - per-connection rate limiting
- Use structured event routing; avoid ad-hoc JSON handling.

### 6.2 Search (Tantivy)
- Index messages by:
  - message_id, guild_id, channel_id, author_id, created_at, content
- Search returns IDs; Postgres fetches full rows.
- Rebuild index job must exist.
- Query DoS protections are required.

### 6.3 LiveKit integration
- Filament is the policy engine.
- LiveKit tokens must be:
  - short TTL
  - scoped to room/channel
  - permissions-limited (publish/subscribe)
- Never trust the client’s requested permissions.

### 6.4 Desktop (Tauri + SolidJS)
- Strict CSP; no remote navigation.
- Minimize Tauri Rust commands.
- Never expose filesystem/network APIs broadly.
- Secure token storage (OS keychain equivalent).

---

## 7) Work Process for Agents

### When implementing a phase/task:
1. Read `PLAN.md` and identify the smallest safe increment.
2. Create or update:
   - tests
   - docs (if behavior changes)
3. Add guardrails:
   - caps, rate limits, timeouts
   - permission checks
4. Run:
   - `cargo fmt`
   - `cargo clippy`
   - `cargo test`
   - security checks (audit/deny) locally if possible

### Commit guidance
- Small commits that pass tests.
- One commit for feature, one for refactor if needed.
- Never commit secrets. Use `.env.example`.

---

## 8) PR Template (Agents should follow)
Include in PR description:
- What changed (1–3 bullets)
- Threat model impact (what new attack surface?)
- Limits added (sizes/rates/queues)
- Tests added/updated
- Any dependency changes + license justification

---

## 9) “Stop and Ask” Triggers
Agents should stop and request maintainer input if:
- Feature requires `unsafe`
- Adding a new cryptography dependency or changing token format
- Changing protocol event compatibility
- Introducing a non-Rust SFU alternative (other than already-approved LiveKit)
- Relaxing limits/timeouts/rate limits for any reason
- Adding new privileged Tauri APIs

---

## 10) Quick Reference Defaults
- Token format: PASETO (short-lived access) + refresh token in DB (hashed)
- DB: Postgres (sqlx)
- Search: Tantivy (rebuildable)
- SFU: LiveKit in docker-compose
- TLS: rustls (or reverse proxy, but rustls-first)
- Markdown: pulldown-cmark → safe UI tokens (no HTML)

---
End of AGENTS.md
