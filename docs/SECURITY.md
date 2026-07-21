# Filament Security Baseline

## Boundary Limits
- HTTP JSON body default cap: `1 MiB`.
- WebSocket frame cap: `64 KiB`.
- WebSocket decoded event cap: `64 KiB`.
- Baseline REST rate limit: `600 requests/minute/client IP` (override with `FILAMENT_RATE_LIMIT_REQUESTS_PER_MINUTE`).
- Auth-route cap (`register/login/refresh`): `60 requests/minute/route+client IP` (override with `FILAMENT_AUTH_ROUTE_REQUESTS_PER_MINUTE`).
- Gateway ingress cap: `60 events/10s/connection` (overrides: `FILAMENT_GATEWAY_INGRESS_EVENTS_PER_WINDOW`, `FILAMENT_GATEWAY_INGRESS_WINDOW_SECS`).
- Media token issuance cap: `60 requests/minute/user+channel+client IP` (override with `FILAMENT_MEDIA_TOKEN_REQUESTS_PER_MINUTE`).
- Media publish churn cap: `24 requests/minute/user+channel+client IP` (override with `FILAMENT_MEDIA_PUBLISH_REQUESTS_PER_MINUTE`).
- Directory join caps: `60 requests/minute/client IP` and `30 requests/minute/authenticated user` (overrides: `FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP`, `FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER`).

## Timeouts
- Default request timeout: `10 seconds`.
- Idle/read/write gateway timeouts are mandatory in gateway implementation phases.

## Logging and Correlation
- Structured JSON logs are required.
- Every request includes an `x-request-id` correlation identifier.
- Security-sensitive events (auth, refresh, moderation, rate-limit violations) must be auditable.
- Key material of any kind (transport, session, or E2EE) must never appear in logs, traces, telemetry, or crash dumps.

## Identity and IDs
- Project-wide identity format is ULID.
- UUID fallback is not used for domain IDs.

## Key Management Policy (PASETO)
Scope: this section governs transport/session token keys. E2EE identity and MLS key management is specified in the End-to-End Encryption baseline below.

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
- Profile banner upload policy (locked for implementation): `6 MiB` cap and MIME allowlist `image/jpeg`, `image/png`, `image/webp`, `image/avif`, `image/gif`.
- Fenced-code highlighting must stay token/AST based (no `innerHTML` or highlighter HTML output path).
- E2EE attachment carve-out: server-side MIME sniffing cannot apply to opaque encrypted blobs; see the End-to-End Encryption baseline for the compensating contract.

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

## End-to-End Encryption (MLS) Baseline
Status: the Phase 0 engineering artifacts, Phase 1 identity/device/KeyPackage
foundation, and initial Phase 2 two-user conversation provisioning, opaque
transport, message mailbox, and recipient-bound commit/Welcome mailbox are
implemented as of 2026-07-21; ADR
ratification is complete, while threat-model ratification remains open.
Packaged-client mailbox processing and complete multi-device MLS leaf handling
remain unfinished, so E2EE is not yet generally available.
Items below remain binding for later phases; the exact completed/remaining
split is maintained in `plans/PLAN_E2EE_IMPL.md`.

### Protocol Stack
- Single vetted stack: MLS (RFC 9420) via OpenMLS for all E2EE domains — 1:1 DMs (2-member groups), group DMs, guild encrypted channels, and calls. No bespoke ratchets, key schedules, or parallel crypto stacks.
- Baseline ciphersuite: `MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519` (0x0003).
- Ciphersuite agility is mandatory in all wire formats and stored state; hybrid post-quantum key establishment (X25519+ML-KEM via HPKE) is a planned fast-follow once standardized and vetted.
- Application messages and commits/proposals are transported as MLS `PrivateMessage`; the server never parses MLS interiors beyond size/shape bounds.
- Randomness from platform CSPRNG only; key material is zeroized on drop and held in platform secure storage where available.

### Crypto Modes — No Escrow
- `conversation_crypto = plaintext | mls_v1` is a conversation invariant; guild channels use `channel_type = plaintext | encrypted`, set at channel creation and permission-gated.
- No per-message crypto toggles, no mixed channels, and no server-readable "encrypted" middle mode. Content is E2EE or honestly plaintext.
- Upgrades are explicit; silent downgrade is prohibited. Capability gaps (any participant without an MLS-capable device) fail closed with a typed error — never a plaintext fallback.

### Identity and Device Keys
- Per-user Ed25519 root identity key; it leaves a device only via QR-mediated encrypted pairing or opt-in passphrase-encrypted backup (Argon2id at aggressive parameters).
- Per-device MLS signature keypair + HPKE init keys. Device certificates `(user_id, device_id, device signature pubkey)` are root-key-signed; MLS leaf credentials embed the certificate and peers verify the chain to the pinned root key.
- QR pairing offers are single-use, expire after at most five minutes, and contain a high-entropy authentication secret plus an ephemeral X25519 receiver key. The existing device signs the pairing context, and the root secret is HPKE-encrypted directly to the new device. Neither the QR offer nor the encrypted transfer is relayed through the Filament server.
- Keys are non-exportable and live in platform keystores (Keychain / Android Keystore / TPM+DPAPI) where available. No private-key display or copy surface exists anywhere in the product.
- Desktop device state uses SQLCipher with a random 32-byte database key held by the OS credential store. Store paths and keys are native-only; webview IPC is limited to initialization and a non-sensitive readiness status. Phase 1 limits the store to 64 MiB, 4,096 records, and 4 MiB per record.
- The server never holds root keys and cannot mint devices; uncertified devices fail verification at every peer.
- Device removal is first-class: MLS Remove of that device's leaves from all groups (cryptographic eviction) plus KeyPackage tombstoning.

### Server-Side Limits (E2EE Endpoints)
- Strict maximum sizes on KeyPackages, commits, Welcomes, proposals, and message envelopes.
- Per-user/per-device/per-route rate limits on KeyPackage upload and claim, commit ingestion, and rekey operations; commit-storm backpressure is required.
- KeyPackage pools are bounded: ordinary packages plus one ordered fallback.
  All currently generated packages are claimed once; reusable last-resort
  behavior is prohibited until an MLS extension implementing it is reviewed.
  Claims are atomic, rate-limited, and audit-logged.
- Delivery Service ordering is single-writer-per-epoch: the first order-valid commit for an epoch is accepted; competing commits receive a deterministic `409 epoch_conflict` rejection.
- Every Welcome is bound to one active target device. Commit delivery is
  snapshotted per active participant device, and only the target device can
  retrieve the Welcome bytes.
- Server-side validation of MLS payloads is shape-only: size bounds, field presence, and epoch monotonicity per group.

### Retention — Mailbox Model
- E2EE ciphertext is retained only until all member devices acknowledge delivery or a TTL expires (default `30 days`, configurable), then hard-deleted. No long-term server-side ciphertext archive.
- The client's local encrypted store (SQLCipher-or-equivalent; store key in platform keystore) is canonical history; the server is a delivery mailbox for E2EE payloads.
- E2EE payloads are padded client-side to size buckets (baseline `512 B / 1 KiB / 4 KiB / 16 KiB`).
- Disappearing-message timers are negotiated inside ciphertext, enforced client-side, and mirrored by server mailbox TTL.
- The server must not store plaintext content, content-derived metadata, or unwrapped key material for `mls_v1` conversations; there are no mixed-mode records.

### Attachments in E2EE Conversations
- Random per-file content key; AEAD aligned with the group suite; no convergent encryption or cross-user deduplication (equality oracle).
- File keys and metadata (filename, MIME, size, content hash, thumbnail key) travel inside MLS application messages; server-side blobs and descriptors are opaque.
- Compensating controls for the MIME-sniff carve-out: hard size caps, per-user quotas, padding buckets, and mailbox TTL still apply server-side; content-type validation moves client-side after decryption.
- No server-side thumbnailing, transcoding, or link unfurling for E2EE content; thumbnails are client-generated and encrypted.

### Client Verification and Delivery Integrity
- Encryption indicators derive only from successful local cryptographic verification. Server-provided fields (`crypto`, `suite`, `epoch`, `sender_device_id`) are routing hints and can never upgrade a message's displayed trust.
- Clients pin peer root keys, surface key-change warnings (blocking interstitial for previously verified contacts), and support safety-number/QR verification.
- Fail closed on: malformed envelopes, unverifiable commits, stale epochs, capability gaps, and any server-field/local-verification mismatch.
- Delivery-gap detection via per-sender MLS generation counters is mandatory ("messages may be missing" indicator).
- Push notifications are data-only; notification text is decrypted on-device; the push pipeline never carries plaintext.
- Web clients are excluded from E2EE in v1; participation requires signed, packaged desktop/mobile builds. The exclusion is a code-delivery property, not a protocol or JS limitation: browser-delivered application code is re-fetched from the operator on every load.
- Web-client rendering of E2EE conversations is a fail-closed capability state only: conversation existence may render (the server already knows membership for routing), content renders as "end-to-end encrypted — open in a packaged client." No plaintext fallback and no server-side decryption path may exist.
- Capability gating is bidirectional: a conversation cannot be created as or upgraded to `mls_v1` unless every participant has at least one MLS-capable device; a web-only participant blocks the upgrade with a typed capability error rather than silently degrading it.

### Packaged Client Architecture
- E2EE-capable clients must bundle UI assets inside the signed package and serve them from the local application protocol. Remote-loading application code from the server (e.g. a desktop shell pointed at the hosted web UI) is prohibited — it reintroduces the web trust model.
- MLS state and all key operations run in the native host process (shared Rust core) behind a narrow, typed IPC surface: commands and ciphertext in, plaintext and verified state out. Key material never enters the webview/JS heap.
- Mobile clients follow the same pattern via FFI bindings to the shared Rust core, with platform keystores for custody.
- Keys are device-bound, never account-bound: valid account credentials in a non-E2EE-capable client (e.g. a browser session) confer no decryption capability; only paired, certified devices decrypt.

### Moderation Contract (E2EE)
- The server is registered as an MLS external sender authorized to propose `Remove` only; clients hard-reject externally proposed `Add`s. The server can shrink a group's read audience, never grow it.
- Kick/ban/role-loss produce Remove commits (cryptographic eviction). Policy enforcement acts immediately (routing stops); eviction lands at the next commit within a bounded window, or clients block sends in that group.
- Workspace policy gate: `encrypted_channel_policy = disabled | require_moderator_membership | unrestricted` (per workspace, optionally per category).
- User reports package the reporter's decrypted copies plus envelope references, with explicit reporter-side disclosure in UX.

### Media E2EE (SFrame over LiveKit)
- All LiveKit token issuance, publish-source, and subscribe policies above remain in force; SFrame layers content confidentiality on top.
- The SFU forwards opaque encrypted frames and cannot decrypt media.
- Media keys derive from the MLS group `exporter_secret`; media epoch equals MLS epoch; rekey on membership commits and periodic update commits.
- Insertable-streams support must be verified per webview target (WebView2, WKWebView, WebKitGTK) before media E2EE ships on that platform; where a webview lacks support, the required fallback is a native WebRTC media path in the host layer — never unencrypted media.

### Directory Audit (E2EE)
- Directory mutations (device certificate publication, KeyPackage pool changes, claims) are audit-logged.
- Audit records contain public material only — never secret key material.

### Supply Chain (E2EE Additions)
- Dependency gate before implementation: `cargo audit` plus `cargo vet` (or equivalent), pinned and hash-locked dependencies, license compatibility gate (OpenMLS is MIT), and external audit status review.
- E2EE-capable clients require signed releases and update-channel integrity (signed manifests, downgrade protection).
- Code signing is necessary but not sufficient: it proves origin, not honesty. It converts per-user, per-load targeted code substitution into a release-pipeline compromise that ships an auditable artifact to all users. Build machines and signing-key custody remain disclosed trust dependencies.
- Reproducible builds let third parties verify that signed binaries match public source; binary transparency for client releases is the roadmap endpoint.
