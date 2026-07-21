# PLAN_E2EE_IMPL.md — Subagent-Based Implementation Plan

**Design source:** [`PLAN_E2EE.md`](PLAN_E2EE.md) (v2.1 — MLS baseline)
**Security contracts:** [`docs/SECURITY.md`](../docs/SECURITY.md) §"End-to-End Encryption (MLS) Baseline", [`docs/THREAT_MODEL.md`](../docs/THREAT_MODEL.md) §"E2EE Threats"
**Project guidelines:** [`AGENTS.md`](../AGENTS.md)
**Baseline:** `PLAN.md` Phases 0–8 complete (auth, gateway, Postgres, Tantivy search, roles/permissions, LiveKit voice/video, desktop hardening, deploy/ops).

---

## Implementation Status — 2026-07-21

The repository is currently at **Phase 1 foundation**, not at usable E2EE
messaging. Plaintext conversations remain the only production message path.

Completed and committed:

- Phase 0 domain types, strict wire DTOs, planned gateway-event contracts,
  threat-model/ADR material, webview capability notes, and a compiling OpenMLS
  lifecycle spike (create, add, application message, update, remove, external
  commit, exporter).
- `filament-e2ee` with Ed25519 root identities, typed device certificates,
  client-side ghost-device rejection, persistent-in-lifetime OpenMLS provider
  state, real KeyPackage generation/Welcome consumption, root-secret keystore
  abstraction, zeroization boundaries, and unit tests.
- Postgres v12 public directory schema and authenticated REST handlers for
  device publication/listing and KeyPackage upload/atomic claim.
- Root-key pinning, server-side certificate verification, exact crypto-field
  and opaque-blob bounds, per-IP/user/device rate limits, pool caps,
  transactionally consistent public audit logging, and single-use claims.
- Irreversible device tombstoning with transactional unclaimed-KeyPackage
  deletion, resurrection rejection, active `device_list_update` and
  `keypackage_low` gateway events, and strict shared-client event decoding.
- Single-use QR device pairing with five-minute/size caps, strict wire parsing,
  existing-device Ed25519 authorization, QR-secret authentication, and
  X25519/ChaCha20-Poly1305 HPKE root-key transfer through the approved OpenMLS
  provider. The server integration test publishes a newly paired device and
  verifies that it shares the pinned root identity.
- Feature-gated production SQLCipher local storage with typed record/store
  identifiers, OS Keychain/Credential Manager/Secret Service key custody,
  symlink/hard-link/path rejection, private Unix permissions, strict
  record/entry/database caps, and native-only initialization/readiness IPC
  contracts. The key-isolation audit is recorded and covered by negative tests.
- Root-identity rotation protocol v1 with dual-signed continuity proofs,
  strictly monotonic replay protection, a bounded public proof chain, atomic
  device revocation and stale-KeyPackage destruction, and native-only pending
  replacement secrets. The packaged settings panel exposes only a fingerprint,
  public device metadata, backup status, and exact typed destructive action.
- Adversarial/integration coverage for forged certificates, root replacement,
  target user/device binding, concurrent claims, fallback exhaustion, device
  removal, live gateway notification delivery, request body limits, and
  configuration limits. The Postgres test runs when
  `FILAMENT_TEST_DATABASE_URL` is configured.

The OpenMLS provider-chain vulnerability blocker is resolved by pinning the
complete OpenMLS crate family to signed upstream fix commit `0e99bc88`. This
selects `hpke-rs 0.7.0` and patched libcrux releases while the project waits for
the next crates.io release; no vulnerability advisory is ignored.

Still required before Phase 1 can be called complete:

- Conversation-scoped peer surfacing of device-list changes follows the Phase
  2 conversation mapping; Phase 1 emits owner-scoped directory notifications.
- The repository's desktop target currently provides the hardened native
  security/IPC layer rather than a runnable Tauri command host. The settings
  view and native rotation state machine are implemented and tested, but final
  runtime command registration awaits that host scaffold.

Phase 2 is now in progress. The client core implements a bounded two-user MLS
conversation lifecycle: claimed-KeyPackage validation against pinned roots,
staged Add/Welcome and safe Remove creation, acceptance-gated commit merge,
strict multi-device Welcome/membership validation, PrivateMessage
encryption/decryption, fail-closed routing-hint checks, and bounded per-sender
generation reordering.
The server now has the v13 Delivery Service foundation for
`mls_v1` conversations: downgrade-protected crypto-mode records, conversation membership
and group mapping, opaque commit/message persistence, deterministic
single-writer epoch ordering, GroupInfo retrieval, exact ciphertext padding
buckets, configurable expiry deadlines, per-IP/user/device/group transport
limits, and active `mls_commit`, `mls_welcome`, and `mls_message` gateway
notifications. The v14 mailbox increment snapshots bounded active-device
deliveries at send time, exposes authenticated/cursor-bounded mailbox reads,
accepts batched acknowledgments only from owned active devices, hard-deletes
ciphertext after every snapshotted device acknowledges, and runs bounded TTL
garbage collection for opaque messages and commits. Missing participant device
capability fails closed with a typed error. The v15 provisioning increment
atomically creates or explicitly upgrades two-user conversations with their
initial commit, Welcome, and GroupInfo; requires active devices for both users;
supports exact idempotent retries; prevents duplicate encrypted user pairs; and
enforces the one-way `plaintext` to `mls_v1` transition in Postgres. The web
client strictly decodes routing notifications but retains no decryption
capability. The v16 commit-mailbox increment binds every Welcome to its exact
active target device, snapshots bounded per-device commit deliveries, exposes
cursor- and byte-bounded offline reads, and hard-deletes commit/Welcome blobs
after all-device acknowledgment or TTL. The native client core now processes
bounded message-mailbox pages with preflight cursor/identifier validation,
fail-closed routing-hint conversion, MLS authentication/decryption, per-entry
failure isolation, generation-gap surfacing, and acknowledgment construction
only for successfully authenticated records. It also processes commit-mailbox
pages as a strict epoch chain: recipient-bound Welcomes establish initial
state, authenticated peer updates advance existing state, membership changes
are limited to one safe device Add or Remove, processing stops at the first
rejected epoch, and acknowledgments cover only the durable success prefix.
Authenticated application padding and
exact transport buckets make generated client ciphertext compatible with the
Delivery Service. The native client now persists a versioned, bounded,
single-record checkpoint containing the complete OpenMLS provider, certified
device signer, pinned roots, group epochs, generation counters, and buffered
generation gaps. Restart restores revalidate all certificates, identifiers,
pins, membership, epochs, and record bounds before returning operational state.
Multi-device churn now adds one root-certified device per recipient-bound
Welcome, processes the same commit without Welcome on existing devices,
enforces 200 total leaves and 100 devices per user, rejects third-user or
duplicate leaves and final-user-device removal, cryptographically evicts
removed devices, and survives durable restart. Packaged runtime wiring, durable
message-history coordination, and external-commit recovery remain to be
implemented. The native commit pipeline now rebases rejected self-update, Add,
and Remove intents on the authenticated single-writer winner, emits a fresh
recipient-bound Welcome for a rebased Add, treats already-applied changes as
satisfied, invalidates changes made unsafe by the winner, and preserves pending
intent across durable restart. Phase 3 and
later attachment, media, guild-channel, hardening, and key-transparency work has
not started.

---

## Execution Model

**Serial subagent execution.** Each phase is dispatched to a subagent that runs to completion, commits, and the next phase is dispatched only after the commit lands. No parallelism between phases.

### Per-Phase Subagent Contract

Each subagent receives:

1. **Goal** — what the phase must accomplish (from the phase section below).
2. **Context** — the full text of the corresponding `PLAN_E2EE.md` phase section, relevant `AGENTS.md` excerpts, the current repo file tree, and integration points listed in each phase.
3. **Deliverables checklist** — concrete artifacts the subagent must produce.
4. **Exit criteria** — gates that must pass before the subagent commits.
5. **Stop-and-ask triggers** — conditions from `AGENTS.md` §9 that require halting for human input.

### Quality Gate (every phase, before commit)

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo audit
cargo deny --config cargo-deny.toml check
```

Phases that add OpenMLS or SQLCipher dependencies may need `cargo vet` configuration (see Phase 0 deliverable).

### Commit Convention

- One commit per phase (or one feature commit + one refactor commit if the subagent restructures).
- Commit message format: `feat(e2ee): phase N — <short description>`
- Branch: `feat/e2ee-implementation`

---

## Codebase Integration Points

### Workspace (`Cargo.toml`)

```
crates/filament-core/          — domain newtypes, invariants, permission engine
crates/filament-protocol/       — gateway envelope { v, t, d }, event manifest
apps/filament-server/           — Axum REST + WS gateway, Postgres, Tantivy, LiveKit
apps/filament-client-desktop/src-tauri/ — Tauri IPC security layer
```

### Server module map (`apps/filament-server/src/server/`)

```
mod.rs              — module declarations
auth.rs             — token auth, IP resolution, message validation
auth_repository.rs  — session/refresh-token persistence
core.rs             — AppState, AppConfig, constants, metrics state
db/                 — schema bootstrap + migrations (v1–v11)
directory_contract.rs — IP network types, join policy constants
domain/             — attachments, reactions domain logic
errors.rs           — AuthFailure error type
gateway_events/     — event builders (connection, message_channel, presence_voice, workspace, profile, friend, envelope)
handlers/           — REST handlers (auth, friends, guilds, media, messages, profile, search)
metrics.rs          — Prometheus counters
permissions.rs      — channel permission snapshot
realtime/           — gateway WS runtime (connection, ingress, fanout, search, voice)
router.rs           — route definitions, rate limit layers, body limits
types.rs            — DTOs (request/response structs, path/query extractors)
```

### Gateway event manifest

`crates/filament-protocol/src/events/gateway_events_manifest.json` — add new `mls_*` and `device_*` event types here with `schema_version`, `scope`, and `lifecycle`.

### Database migrations

`apps/filament-server/src/server/db/migrations/` — numbered `v1` through `v11`. New E2EE schema additions continue as `v12_e2ee_*`, `v13_*`, etc.

### Existing protocol patterns

- Envelope: `Envelope<T> { v: u16, t: EventType, d: T }` — all new events use this.
- `EventType` validation: lowercase ASCII + digits + `_` and `.`, max 64 chars.
- All DTOs use `#[serde(deny_unknown_fields)]` for strict boundary parsing.
- `PROTOCOL_VERSION = 1`, `MAX_EVENT_BYTES = 64 KiB`.

### Cargo deny license allowlist

`cargo-deny.toml` allows: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib. OpenMLS is MIT — compatible. Any new dependency must pass this gate.

---

## Open Decisions to Lock in Phase 0

These are from `PLAN_E2EE.md` §"Open Decisions". The subagent for Phase 0 should draft recommendations; the maintainer ratifies before Phase 1 begins.

| # | Decision | Recommended | Rationale |
|---|-----------|-------------|-----------|
| 1 | Backup default | Opt-in passphrase backup | Hard no-backup kills adoption; Signal relented |
| 2 | Deniability | Accept MLS in-group non-repudiation | Record in ADR; deniable auth is rejected complexity for v1 |
| 3 | Message franking | v1.5 candidate | Decide after Phase 3 telemetry |
| 4 | Guild encrypted channel size ceiling | Hard cap 5k leaves initially | Uncapped deferred until perf gates prove out |
| 5 | Web client future | Permanent exclusion for v1; revisit as disclosed-degraded tier | No viable browser trust model against adversary #1 |
| 6 | Baseline suite | 0x0003 (X25519/ChaCha20-Poly1305) | Library maturity + performance |
| 7 | Read receipts / typing in E2EE | Off by default; inside ciphertext if shipped | Metadata cost |
| 8 | KT construction | Defer to Phase 8 design | VRF vs CT log — evaluate then |
| 9 | Moderation bots in encrypted channels | Allow (workspace opt-in, disclosed) | Visible member, not hidden recipient |

---

## Phase 0 — Design Lock & Engineering Spikes

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 0"

### Goal

Lock all design decisions into ratified documents and wire contracts. Produce the ADR, update the threat model, define all API/event schemas, run engineering spikes to de-risk OpenMLS integration. No production code beyond the ADR, docs, wire-contract types, and spike harnesses.

### Deliverables

1. **ADR document** at `docs/adr/0001-e2ee-mls-openmls.md`:
   - OpenMLS selection + ciphersuite 0x0003 + agility plan
   - Rejected alternatives with rationale (libsignal: AGPL-3.0 + infra coupling; vodozemac: weak group PCS, no membership auth, no exporter, no PQ path; static per-device: no FS/PCS against archive adversary)
   - MLS in-group non-repudiation acceptance (Open Decision 2)
   - Web-client exclusion with code-delivery rationale
   - Packaged-client architecture (bundled assets, Rust-core crypto behind IPC, device-bound keys)
   - Mailbox retention model + backup policy (Open Decision 1)
   - OpenMLS dependency license justification (MIT — passes cargo-deny)

2. **Threat model update** — merge the `PLAN_E2EE.md` threat model into `docs/THREAT_MODEL.md` E2EE section (partially present; verify completeness and ratify).

3. **Wire contracts** in `crates/filament-protocol/`:
   - DTO structs for all E2EE endpoints (see `PLAN_E2EE.md` §"API/Protocol Additions"):
     - `PUT /e2ee/devices/{device_id}` — device certificate publish
     - `GET /e2ee/users/{user_id}/devices` — certified device list
     - `POST /e2ee/keypackages` — KeyPackage pool upload
     - `POST /e2ee/keypackages/claim` — KeyPackage claim
     - `GET /e2ee/groups/{group_id}/info` — encrypted GroupInfo
     - `POST /e2ee/groups/{group_id}/commits` — Delivery Service commit ingestion
     - `POST /e2ee/groups/{group_id}/messages` — PrivateMessage transport
   - Gateway event data types for: `mls_message`, `mls_commit`, `mls_welcome`, `mls_proposal`, `device_list_update`, `keypackage_low`
   - Add all new event types to `gateway_events_manifest.json`
   - All structs use `#[serde(deny_unknown_fields)]` and enforce size bounds at parse time

4. **E2EE domain types** in `crates/filament-core/`:
   - `DeviceId` (ULID newtype)
   - `DeviceCertificate` (struct with invariant validation)
   - `GroupId` (MLS group identifier newtype)
   - `ConversationCrypto` enum (`Plaintext | MlsV1`)
   - `CiphersuiteId` (u16 newtype, validates against known suite IDs)
   - `EpochTag` (newtype for MLS epoch)
   - Unit tests for all newtype invariants

5. **Engineering spike: OpenMLS 2-member group round trip** in `spikes/e2ee-mls-roundtrip/`:
   - Rust CLI harness (separate crate, not in workspace members)
   - Create group, add member, send message, remove member, external-commit recovery
   - Against a fixture in-process Delivery Service
   - Validates OpenMLS API surface and identifies integration patterns

6. **Engineering spike: insertable-streams verification** in `spikes/e2ee-webview-check/`:
   - Verify `RTCRtpScriptTransform` / insertable-streams availability in WKWebView and WebKitGTK
   - Document results; informs Phase 5 media path

7. **Cargo vet / supply chain setup**:
   - Add `cargo vet` configuration (or document equivalent review process)
   - Audit OpenMLS transitive dependency tree against cargo-deny license allowlist
   - Document any advisory exceptions needed (like the existing `RUSTSEC-2024-0384` pattern)

### Exit Criteria

- [x] ADR approved by maintainer
- [ ] `docs/THREAT_MODEL.md` E2EE section ratified
- [ ] Wire contract types compile, parse with strict validation, and have unit tests
- [ ] Gateway event manifest includes all new `mls_*` and `device_*` event types
- [x] OpenMLS spike demonstrates full 2-member lifecycle (create, add, message, remove, external-commit recovery)
- [ ] Insertable-streams spike documents per-webview availability
- [x] Supply-chain gate configured for OpenMLS dependencies

### Stop-and-Ask Triggers

- Adding OpenMLS as a new cryptography dependency → requires maintainer approval (AGENTS.md §9)
- Changing protocol event compatibility → requires maintainer signoff (AGENTS.md §9)

### Integration Points for Phase 1

- Wire contract types in `filament-protocol` → Phase 1 implements the server handlers
- Domain types in `filament-core` → Phase 1 uses `DeviceId`, `DeviceCertificate`, `GroupId`
- OpenMLS spike patterns → Phase 1 production crate structure
- ADR decisions → binding for all subsequent phases

---

## Phase 1 — Identity, Devices, KeyPackages

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 1", §"Identity and Device Model", §"KeyPackages"

### Goal

Implement the full identity and device management layer: root identity keys, device certificates, KeyPackage pools, QR device pairing, and the local encrypted store foundation. This is the crypto foundation that all subsequent phases build on.

### Deliverables

1. **New crate: `crates/filament-e2ee/`** — MLS client core:
   - OpenMLS integration (group state management, KeyPackage generation, commit/proposal processing)
   - Root identity key generation (Ed25519)
   - Device certificate creation and verification (root-key-signed)
   - KeyPackage pool management (single-use + one ordered, single-use fallback;
     reusable MLS last-resort semantics require a separately reviewed extension)
   - Key material zeroization on drop (`zeroize` crate)
   - Platform CSPRNG only
   - `#![forbid(unsafe_code)]` enforced

2. **Server endpoints** in `apps/filament-server/src/server/handlers/e2ee.rs` (new module):
   - `PUT /e2ee/devices/{device_id}` — publish device certificate
     - Validates certificate signature against published root key
     - Rate-limited per user
   - `GET /e2ee/users/{user_id}/devices` — certified device list
     - Returns public material only; clients verify signatures
   - `POST /e2ee/keypackages` — upload KeyPackage pool
     - Bounded pool size, per-device rate limit
   - `POST /e2ee/keypackages/claim` — claim KeyPackage for target user/device
     - Rate-limited, audit-logged, atomically decrements pool
   - Register module in `handlers/mod.rs` and routes in `router.rs`
   - Add to `ROUTE_MANIFEST` test array

3. **Database schema** — migration `v12_e2ee_identity`:
   - `e2ee_device_certificates` (user_id, device_id, certificate_blob, root_key_pub, created_at, tombstoned_at)
   - `e2ee_keypackages` (device_id, keypackage_blob, is_last_resort, claimed_at, created_at)
   - `e2ee_audit_log` (action, user_id, device_id, metadata_json, created_at) — public material only
   - All MLS blobs stored as opaque `BYTEA`; server never parses interiors

4. **QR device pairing flow**:
   - Encrypted key transfer from existing device to new device
   - Device addition signed by existing device's key
   - In-conversation surfacing event (`device_list_update` gateway event)
   - KeyPackage tombstoning on device removal

5. **Local encrypted store foundation** (client-side, in `filament-e2ee` crate):
   - SQLCipher (or equivalent) encrypted message store abstraction
   - Store key management interface (platform keystore integration point)
   - Foundation only — full history sync comes in Phase 4

6. **Encryption settings panel** (desktop client, SolidJS):
   - Safety number / root key fingerprint display (shareable, QR)
   - Device list (name, added date, verification state, Remove device action)
   - Rotate identity action (destructive, typed confirmation)
   - Backup enrollment status and controls
   - No private-key display or copy surface anywhere

7. **Security controls**:
   - KeyPackage pool size caps (per device, per user)
   - KeyPackage claim rate limits (per user, per device, per IP)
   - Claim audit logging (public material only — never secret material)
   - Device certificate rate-limited publication
   - Zero key material in logs, telemetry, tracing, or crash dumps

8. **Tests**:
   - Unit: device certificate verification, KeyPackage pool invariant enforcement, newtype validation
   - Integration: device publish, KeyPackage claim, rotation, pairing — deterministic with fixture data
   - **Negative test:** server-forged device certificate is rejected by clients (ghost-device injection fails)
   - **Key-isolation audit:** MLS key material confined to Rust core; webview has no key access path (IPC surface review + negative test)

### Exit Criteria

- [x] Deterministic integration tests pass for device publish, KeyPackage claim, rotation, and pairing
- [x] Ghost-device injection negative test passes (server-forged certificate rejected)
- [x] Key-isolation audit passes (IPC surface review + negative test confirms no key access from webview)
- [ ] All quality gates pass (fmt, clippy, test, audit, deny)
- [x] Rate limits and audit logging verified in integration tests

### Stop-and-Ask Triggers

- OpenMLS integration patterns that require `unsafe` → stop and ask
- Platform keystore API that requires privileged Tauri commands → stop and ask (AGENTS.md §9)

### Integration Points for Phase 2

- `filament-e2ee` crate's MLS group operations → Phase 2 uses for 2-member groups
- Device certificate verification → Phase 2 verifies peer devices before group creation
- KeyPackage claim endpoint → Phase 2 uses to bootstrap MLS group creation
- Local encrypted store → Phase 2 stores decrypted message history

---

## Phase 2 — 1:1 DM E2EE (2-Member MLS Groups)

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 2", §"Conversation Types and Crypto Modes", §"Data Model (Server-Side)"

### Goal

Implement the first end-to-end encrypted conversation type: 1:1 DMs as 2-member MLS groups. This proves the full message transport pipeline — encryption, server relay of opaque blobs, client-side decryption and verification — and establishes the mailbox delivery model.

### Deliverables

1. **MLS group lifecycle** in `filament-e2ee` crate:
   - Create 2-member MLS group from a KeyPackage claim
   - `PrivateMessage` encryption and decryption
   - Commit pipeline with epoch-conflict detection and rebase
   - Per-sender generation counter tracking (delivery-gap detection)
   - External-commit recovery from desync

2. **Conversation crypto mode** integration:
   - `conversation_crypto = plaintext | mls_v1` field on DM conversations
   - Create/upgrade flow: user-explicit enable at creation or upgrade
   - No silent downgrade ever; "downgrade" = explicitly creating a new plaintext conversation
   - Capability gating: both participants must have at least one MLS-capable device; otherwise typed capability error; fail closed

3. **Server endpoints** — extend `handlers/e2ee.rs`:
   - `POST /e2ee/groups/{group_id}/commits` — Delivery Service commit ingestion
     - Single-writer-per-epoch: first order-valid commit for epoch N accepted; competing commits get `409 epoch_conflict`
     - Shape-only validation: size bounds, field presence, epoch monotonicity
   - `POST /e2ee/groups/{group_id}/messages` — `PrivateMessage` transport
     - Opaque blob storage; server never parses MLS interiors
     - Size-bucket padding verification (512 B / 1 KiB / 4 KiB / 16 KiB)
   - `GET /e2ee/groups/{group_id}/info` — encrypted GroupInfo for joins/recovery

4. **Mailbox delivery model**:
   - Per-device delivery acknowledgments
   - Mailbox GC: hard-delete after all-device ack or TTL (default 30 days, configurable)
   - TTL-based GC background task
   - No long-term ciphertext archive

5. **Gateway events** — add builders in `gateway_events/`:
   - `mls_message` — new encrypted message in a group
   - `mls_commit` — membership/state update commit
   - `mls_welcome` — new member welcome
   - `mls_proposal` — pending proposal
   - `device_list_update` — peer device list changed
   - `keypackage_low` — device KeyPackage pool below water mark
   - All events inside `{ v, t, d }` envelope with strict bounds

6. **Wire fields on E2EE message records**:
   - `crypto` (plaintext|mls_v1), `suite`, `epoch`, `sender_device_id` — routing hints only
   - Clients derive all trust state from local MLS verification; server fields can never upgrade displayed trust

7. **Client-side features**:
   - Key-change warnings (passive indicator; blocking interstitial for previously-verified contacts)
   - Gap indicators ("messages may be missing" when generation counters have gaps past threshold)
   - Mailbox acks sent by client on successful decryption

8. **Database schema** — migration `v13_e2ee_messages`:
   - `e2ee_messages` (message_id, group_id, sender_device_id, epoch, suite_id, ciphertext_blob, created_at_unix)
   - `e2ee_message_acks` (message_id, device_id, acked_at)
   - `e2ee_groups` (group_id, conversation_id, current_epoch, group_info_blob, created_at)
   - All ciphertext stored as opaque `BYTEA`

9. **Tests**:
   - Two-device 1:1 churn test: create, message, epoch advance, message, verify decryption
   - Multi-device 1:1 churn: user with 2 devices, both receive and decrypt
   - Out-of-order delivery: messages arrive in wrong order, client reorders by generation
   - Offline catch-up: device comes online, fetches mailbox, decrypts, acks
   - Epoch conflict: two concurrent commits, deterministic `409` rejection, client rebases
   - **Persistence audit:** server records for E2EE fixtures contain opaque envelopes only (no plaintext content, no key material)
   - Capability gating: participant without MLS-capable device → typed error, no fallback
   - Downgrade attempt: server flips `crypto` hint → client fails closed, never falls back to plaintext

### Exit Criteria

- [x] Two-device and multi-device 1:1 churn tests pass, including out-of-order and offline catch-up
- [x] Persistence audit confirms server stores opaque envelopes only
- [x] Epoch-conflict handling is deterministic and tested
- [x] Mailbox ack and GC verified (all-device ack triggers delete; TTL triggers delete)
- [x] Capability gating fails closed with typed error
- [ ] Downgrade attempts surface warnings / fail closed, never fall back
- [ ] All quality gates pass

### Stop-and-Ask Triggers

- Protocol event compatibility changes (new event types are additive, but if parsing changes affect existing events → stop and ask)

### Integration Points for Phase 3

- MLS group operations → Phase 3 extends to N-member groups
- Commit pipeline with epoch-conflict → Phase 3 adds membership proposals/commits
- Mailbox model → Phase 3 reuses for group DMs
- Gateway event infrastructure → Phase 3 reuses `mls_*` events

---

## Phase 3 — Group DM E2EE

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 3", §"Audience Model"

### Goal

Extend E2EE from 2-member DMs to N-member group DMs. Add MLS membership proposals/commits, join via Welcome, removal as cryptographic eviction, and external-commit recovery from desync.

### Deliverables

1. **MLS membership operations** in `filament-e2ee`:
   - Add proposal: member invites new participant, commits with Welcome
   - Remove proposal: member removes participant → cryptographic eviction
   - Update proposal: device rekey (post-compromise security)
   - External-commit recovery: client detects desync, fetches GroupInfo, re-joins via external commit
   - Membership change surfacing: "X added Y", "X removed Y" in-conversation notifications

2. **Server-side Delivery Service enhancements**:
   - Process Add/Remove/Update proposals and commits
   - Welcome message relay (opaque blob, server never parses interior)
   - External sender registration: server as MLS external sender authorized for Remove proposals only
   - Clients hard-reject any externally-proposed Add (ghost-member defense)
   - Bounded reconciliation window for membership state

3. **Audience model enforcement**:
   - Audience = MLS group membership — nothing else
   - No per-message recipient editing, no per-send role-expansion selectors, no manual device picking
   - Membership changes only via member-signed MLS proposals/commits
   - Stale/ambiguous state: client refreshes, rebases, fails closed rather than sending under uncertainty

4. **Message-adjacent features** (inside ciphertext):
   - Reactions, edits, delete-for-everyone, replies/quote previews, pins: all MLS application messages
   - Server never learns their semantics
   - Link previews: client-generated only, per-conversation opt-in, off by default
   - Read receipts and typing indicators: off by default; if shipped, inside ciphertext

5. **Tests**:
   - Membership churn: add member, verify they can decrypt; remove member, verify post-removal epochs are unreadable to them
   - Concurrent commit races: two members commit simultaneously, deterministic resolution via `409 epoch_conflict`
   - Desync self-healing: client with stale state recovers via external commit
   - External-sender Remove: server proposes remove, online clients validate and auto-commit
   - External-sender Add: server proposes add, clients hard-reject
   - Ghost-member injection: server attempts to inject member via forged proposal → signature verification fails at every client
   - Message-adjacent features: reaction, edit, delete, reply all work inside encrypted group
   - Large group stress: 20-member group, membership churn, message delivery

### Exit Criteria

- [ ] Removed members fail to decrypt all post-removal epochs (cryptographic eviction verified)
- [ ] Concurrent commit races resolve deterministically
- [ ] Desync self-heals via external commit
- [ ] External-sender Remove proposals are validated and auto-committed by clients
- [ ] External-sender Add proposals are hard-rejected by clients
- [ ] Ghost-member injection fails at every client
- [ ] Message-adjacent features (reactions, edits, deletes, replies) work inside encrypted groups
- [ ] All quality gates pass

### Integration Points for Phase 4

- MLS group operations (N-member) → Phase 4 reuses for attachment encryption
- External-commit recovery → Phase 4 reuses for history sync desync recovery
- Mailbox model → Phase 4 extends for encrypted attachment blobs

---

## Phase 4 — Attachments, History, Search

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 4", §"Attachments (E2EE conversations)", §"History, Storage, and Retention"

### Goal

Add encrypted attachments, device-to-device history sync, opt-in passphrase backup, disappearing messages, and local search — completing the E2EE text experience.

### Deliverables

1. **Encrypted attachment flow**:
   - Random per-file content key; AEAD aligned with group suite (ChaCha20-Poly1305)
   - No convergent encryption or cross-user deduplication (equality oracle avoidance)
   - File key + metadata (filename, MIME, size, content hash, thumbnail key) inside MLS application message
   - Server-side blob and descriptor are opaque
   - Client-generated encrypted thumbnails; no server-side thumbnailing/transcoding/unfurling
   - Blob storage follows mailbox model: padded to size buckets, deleted after all-device fetch or conversation TTL
   - Compensating controls for MIME-sniff carve-out: hard size caps, per-user quotas, padding buckets, mailbox TTL still apply server-side; content-type validation moves client-side after decryption

2. **Device-to-device history sync**:
   - New device onboarding restores history from an existing device (encrypted transfer)
   - QR pairing alone grants future messages only — history sync is separate
   - Encrypted transfer protocol between paired devices
   - No server-side plaintext at any point in the sync

3. **Opt-in passphrase backup** (Open Decision 1 → opt-in):
   - Passphrase-encrypted blob (Argon2id at aggressive parameters)
   - Covers identity keys + history snapshot
   - Clearly documented non-recoverability if passphrase lost
   - Backup enrollment controls in encryption settings panel

4. **Disappearing messages**:
   - Per-conversation timer, negotiated inside ciphertext
   - Enforced client-side (local deletion after timer)
   - Mirrored by server mailbox TTL

5. **Local search index**:
   - Client-side full-text index built from local plaintext store
   - Tantivy in desktop client (or SQLite FTS5 on mobile)
   - Encrypted at rest
   - Replaces server-side Tantivy for E2EE conversations (server-side search returns nothing for `mls_v1` conversations)

6. **Tests**:
   - New-device onboarding: device pairs via QR, syncs history from existing device, can decrypt all past messages — no server-side plaintext involved
   - Encrypted attachment: upload encrypted blob, server stores opaque, recipient decrypts, MIME/content verified client-side
   - Server inspection: encrypted files remain opaque (no plaintext content, no content-derived metadata)
   - Mailbox GC for attachments: all-device fetch triggers delete; TTL triggers delete
   - Backup: create passphrase-encrypted backup, restore on new device, verify identity keys + history
   - Disappearing messages: timer expires, client deletes locally, server mailbox TTL also expires
   - Local search: index built from decrypted local store, search returns results without server involvement

### Exit Criteria

- [ ] New-device onboarding restores history without any server-side plaintext
- [ ] Encrypted files remain opaque to server inspection
- [ ] Mailbox GC verified for both messages and attachments
- [ ] Backup create + restore verified
- [ ] Disappearing messages enforced client-side and mirrored by server TTL
- [ ] Local search works without server involvement for E2EE conversations
- [ ] All quality gates pass

### Integration Points for Phase 5

- MLS exporter secrets → Phase 5 uses for SFrame media key derivation
- Local encrypted store → Phase 5 stores media key material
- Mailbox model → Phase 5 may reference for media key delivery

---

## Phase 5 — Voice/Video E2EE

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 5", §"Voice/Video E2EE Direction"

### Goal

Add E2EE for voice/video via SFrame keyed from MLS exporter secrets. The SFU (LiveKit) forwards opaque encrypted frames and cannot decrypt media.

### Deliverables

1. **SFrame integration**:
   - SFrame media encryption over insertable streams
   - Keys derived from MLS group `exporter_secret` of the corresponding group epoch
   - Media epoch == MLS epoch
   - Rekey on participant join/leave (membership commit) and periodic update commits

2. **LiveKit opaque-forwarding path**:
   - SFU forwards encrypted frames only — cannot decrypt media
   - Existing token issuance, publish-source, and subscribe policies remain in force
   - SFrame layers content confidentiality on top of existing transport security

3. **Webview verification matrix** (from Phase 0 spike results):
   - WebView2 (Chromium): verify insertable-streams / `RTCRtpScriptTransform` support
   - WKWebView (macOS/iOS): verify support
   - WebKitGTK (Linux): verify support
   - Where a webview lacks support: native WebRTC media path in the host layer (Rust core)
   - Shipping unencrypted media is never the fallback

4. **Desktop client integration**:
   - SFrame encryption/decryption in Rust host process (not in webview)
   - Encrypted media frames cross IPC boundary as opaque data
   - Key material stays in Rust core, never enters JS heap

5. **Tests**:
   - SFU relays encrypted media only — verify SFU cannot decrypt frames
   - Decryption exclusively at endpoints — sender encrypts, receiver decrypts, SFU forwards opaque
   - Join/leave rekey: new participant joins, media rekeys, old keys don't decrypt new frames; participant leaves, remaining members rekey, leaver can't decrypt
   - Periodic update commit rekey: interval timer fires, media rekeys
   - Insertable-streams matrix: document per-platform support; exercise native fallback on any platform lacking webview support
   - Multi-participant call: 3+ participants, all encrypt/decrypt correctly, SFU forwards all

### Exit Criteria

- [ ] SFU relays encrypted media only; decryption exclusively at endpoints
- [ ] Join/leave rekey verified
- [ ] Periodic update commit rekey verified
- [ ] Insertable-streams verification matrix complete (WebView2 / WKWebView / WebKitGTK)
- [ ] Native media path exercised on any platform lacking webview support
- [ ] No unencrypted media fallback exists
- [ ] All quality gates pass

### Stop-and-Ask Triggers

- If webview lacks insertable-streams and the native WebRTC fallback requires a non-Rust media path → stop and ask (AGENTS.md §9: "Introducing a non-Rust SFU alternative")

### Integration Points for Phase 6

- MLS exporter secret derivation → Phase 6 reuses for guild encrypted channel calls
- SFrame integration patterns → Phase 6 reuses for large-group media
- Rekey on membership changes → Phase 6 reuses for guild channel role-loss eviction

---

## Phase 6 — Guild Encrypted Channels

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 6", §"Conversation Types and Crypto Modes" (guild channels), §"Audience Model" (guild reconciliation)

### Goal

Add `channel_type = encrypted` for guild channels with permissioned Add/Remove commit flows that reconcile channel authorization to MLS group membership. Large-group performance work for 10³–10⁴ leaves.

### Deliverables

1. **Encrypted channel type**:
   - `channel_type = plaintext | encrypted`, set at channel creation, permission-gated
   - Entire channel is one mode — no per-message crypto toggles, no mixed channels
   - `encrypted_channel_policy = disabled | require_moderator_membership | unrestricted` (per workspace, optionally per category)
   - Default guild behavior remains plaintext for full moderation/search viability
   - Open Decision 4: hard cap at 5,000 leaves initially (or perf-gated uncapped — lock in Phase 0 ADR)

2. **Authorization-to-membership reconciliation**:
   - Joining a channel = Add proposal committed by an authorized member/admin device per channel permissions
   - Leave/kick/role-loss = Remove commit
   - Permission changes reconcile to commits promptly (bounded reconciliation window, monitored)
   - Server enforces WHO MAY PROPOSE (policy); clients enforce WHAT IS CRYPTOGRAPHICALLY VALID (signatures, epoch state) — both must pass
   - Reconciliation background task: monitors permission changes, triggers proposal generation

3. **Large-group performance**:
   - Tree operations benchmarked at 10³–10⁴ leaves
   - OpenMLS ratchet tree efficiency verified
   - Commit processing latency within target budget at scale
   - Memory usage bounded at scale

4. **Moderation in encrypted channels** (§"Moderation, Abuse, and Reporting"):
   - Layer 1 (structural): workspace policy, channel permissions, slowmode, freeze — unchanged, server-side
   - Layer 2 (membership): kick/ban/role-loss = Remove commit = cryptographic eviction
   - Layer 3 (content): moderators who are members see plaintext; `require_moderator_membership` makes this a channel invariant
   - Server-initiated removal: external sender proposes Remove; online clients validate and auto-commit
   - Removal latency bounded: if required Remove commit not ordered within bounded window, clients block sends and surface warning

5. **Capability gating for guild channels**:
   - Encrypted channel requires every participant to have at least one MLS-capable device
   - Web-only participants block creation/join with typed capability error
   - Encrypted channels fail closed on unsupported clients

6. **Tests**:
   - Role-loss eviction: user loses role, Remove commit processed within reconciliation window, post-removal epochs unreadable
   - Performance: tree operations at 1k and 5k leaves, commit latency within budget
   - Capability gating: web-only user blocked from joining encrypted channel with typed error
   - Moderation: moderator member can read; non-member moderator cannot read (if `require_moderator_membership`)
   - Server-initiated Remove: server proposes removal, clients validate and commit; Add proposal from server hard-rejected
   - Reconciliation: permission change triggers proposal within bounded window
   - Fail-closed: unsupported client attempts to access encrypted channel → typed error, no plaintext fallback

### Exit Criteria

- [ ] Role-loss eviction tests pass within the reconciliation window
- [ ] Performance budget met at target channel size (1k–5k leaves)
- [ ] Encrypted channels fail closed on unsupported clients
- [ ] Moderation model verified (member-moderator reads, non-member doesn't, server Remove works, server Add rejected)
- [ ] Reconciliation window enforced and tested
- [ ] All quality gates pass

### Stop-and-Ask Triggers

- If performance requirements demand relaxing limits or increasing body size caps → stop and ask (AGENTS.md §9: "Relaxing limits/timeouts/rate limits")

### Integration Points for Phase 7

- Full E2EE feature set → Phase 7 fuzzes and load-tests all of it
- Performance benchmarks → Phase 7 stress-tests at and beyond targets

---

## Phase 7 — Hardening and GA

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 7", §"Security Controls and Limits", §"Supply Chain and Build Integrity"

### Goal

Fuzz the MLS ingestion paths, load-test commit storms and KeyPackage exhaustion, run adversarial review of QR pairing, finalize UX/docs/trust disclosures, and prepare for external security review.

### Deliverables

1. **Fuzzing targets**:
   - MLS envelope parsing (malformed `PrivateMessage`, commits, Welcomes, proposals)
   - Commit/state handling (malformed GroupInfo, ratchet tree corruption)
   - Device certificate verification (forged signatures, malformed certificates)
   - KeyPackage parsing (malformed init keys, invalid suite IDs)
   - Fuzz targets in `crates/filament-e2ee/fuzz/` using `cargo fuzz`

2. **Load tests**:
   - Commit-storm: many concurrent commits against a single group, verify backpressure and `epoch_conflict` determinism
   - KeyPackage-exhaustion: flood claims against a device's pool, verify rate limiting and pool depletion handling
   - Large-group churn: rapid add/remove cycles at 1k+ leaves, verify no state corruption
   - Mailbox GC under load: high message volume, verify GC keeps up

3. **Adversarial review of QR pairing protocol**:
   - Man-in-the-middle attack vectors
   - Malicious or coerced pairing attempts
   - Key exfiltration during pairing transfer
   - Document and fix any findings

4. **UX finalization**:
   - Key-change interstitials (blocking for previously verified contacts)
   - Capability errors (typed, actionable messages)
   - Disappearing-message enforcement UX
   - Safety-number/QR verification flow polish
   - "Messages may be missing" indicator UX

5. **Trust disclosures and documentation**:
   - In-app trust disclosure: TOFU for first contact, residual risks documented
   - `docs/CLIENT_SECURITY.md` E2EE section: IPC boundary, key isolation, webview trust model
   - Operational runbooks: KeyPackage pool monitoring, mailbox GC monitoring, commit-storm response
   - `docs/DEPLOY.md` E2EE section: encrypted conversation configuration, mailbox TTL settings, backup policy

6. **Supply chain hardening**:
   - `cargo vet` fully configured for OpenMLS and all E2EE dependencies
   - Pinned and hash-locked dependencies verified
   - External audit status review documented
   - Signed update manifests with downgrade protection (design + implementation for desktop updater)
   - Reproducible build documentation

7. **Tests**:
   - Fuzz targets run for minimum duration without panics
   - Load tests meet performance budgets (commit-storm backpressure, KeyPackage exhaustion handling, large-group churn)
   - Adversarial QR pairing tests: MITM detected, coerced pairing fails safely
   - All existing E2EE tests still pass (regression)

### Exit Criteria

- [ ] External security review signoff
- [ ] Operational runbooks complete
- [ ] Fuzz targets run clean for minimum duration
- [ ] Load tests meet performance budgets
- [ ] Adversarial QR pairing review complete with fixes for any findings
- [ ] Trust disclosures shipped in-app and in docs
- [ ] All quality gates pass

### Stop-and-Ask Triggers

- Any fuzzing finding that reveals a fundamental protocol or design flaw → stop and ask
- External security review findings that require design changes → stop and ask

### Integration Points for Phase 8

- Hardened E2EE implementation → Phase 8 adds key transparency on top
- Supply chain vetting → Phase 8 extends for KT log infrastructure
- Operational runbooks → Phase 8 adds KT monitoring

---

## Phase 8 — Key Transparency and PQ

**PLAN_E2EE.md reference:** §"Rollout Phases → Phase 8", §"Key Transparency (roadmap)", §"Cryptographic parameters" (PQ)

### Goal

Add an append-only, auditable key transparency log with inclusion and consistency proofs, and adopt a hybrid post-quantum ciphersuite (X25519+ML-KEM) once standardized and vetted.

### Deliverables

1. **Key transparency log**:
   - Append-only, auditable log of the key directory (root keys, device certificates)
   - Inclusion proofs: client can verify their key is in the log
   - Consistency proofs: client can verify the log has only grown (no tampering with history)
   - Client-side auditing: verify inclusion + consistency on every directory lookup
   - Out-of-band checkpoint distribution (e.g., signed checkpoints published via multiple channels)
   - Open Decision 8: KT construction (CT-style static log vs. VRF-based CONIKS-style directory) — lock design in this phase

2. **Equivocation detection**:
   - Server serves different key sets to different users → inconsistency proofs reveal it
   - Server suppresses a rotation → missing entry detected by consistency check
   - Converts "server can lie silently" into "server can lie once and get caught"

3. **Hybrid post-quantum ciphersuite**:
   - Adopt X25519+ML-KEM via HPKE (X-Wing-style) once standardized and vetted
   - Ciphersuite agility in wire formats and stored state (designed in Phase 0)
   - Suite migration: exercise on fixture groups (create with 0x0003, migrate to hybrid)
   - PQ rekeying (Level 3) as fast-follow after initial PQ key establishment (Level 2)

4. **Tests**:
   - Equivocation detection: server serves different keys to two clients, inconsistency proof reveals it
   - Inclusion proof: client verifies their key is in the log
   - Consistency proof: client verifies log growth is append-only
   - Checkpoint verification: out-of-band checkpoint matches log head
   - Suite migration: fixture group created with 0x0003, migrated to hybrid PQ suite, all messages decrypt
   - PQ rekey: periodic PQ rekey on fixture group, forward secrecy maintained

### Exit Criteria

- [ ] Equivocation detection demonstrated in tests
- [ ] Inclusion and consistency proofs verified
- [ ] Suite migration exercised on fixture groups (0x0003 → hybrid PQ)
- [ ] All quality gates pass

### Stop-and-Ask Triggers

- Adding a new cryptography dependency for ML-KEM / PQ → stop and ask (AGENTS.md §9)
- Changing protocol event compatibility for KT proofs → stop and ask (AGENTS.md §9)

---

## Dependency Chain

```
Phase 0 (Design Lock)
  └→ Phase 1 (Identity, Devices, KeyPackages)
       └→ Phase 2 (1:1 DM E2EE)
            └→ Phase 3 (Group DM E2EE)
                 └→ Phase 4 (Attachments, History, Search)
                      └→ Phase 5 (Voice/Video E2EE)
                           └→ Phase 6 (Guild Encrypted Channels)
                                └→ Phase 7 (Hardening and GA)
                                     └→ Phase 8 (Key Transparency and PQ)
```

Each phase strictly depends on the previous. No phase may begin until the prior phase's commit has landed and quality gates have passed.

---

## Cross-Cutting Concerns

### New Crates

| Crate | Phase | Purpose |
|-------|-------|---------|
| `crates/filament-e2ee` | 1 | OpenMLS integration, MLS group operations, key management, device certificates |
| `spikes/e2ee-mls-roundtrip` | 0 | Throwaway spike for OpenMLS API validation |
| `spikes/e2ee-webview-check` | 0 | Throwaway spike for insertable-streams verification |

### Database Migrations

| Migration | Phase | Tables |
|-----------|-------|--------|
| `v12_e2ee_identity` + `v12_e2ee_root_rotation` | 1 | identity roots/rotations, device certificates, KeyPackages, public audit log |
| `v13_e2ee_messages` | 2 | `e2ee_messages`, `e2ee_message_acks`, `e2ee_groups` |
| `v14_e2ee_mailbox` | 2 | pending per-device message deliveries |
| `v15_e2ee_conversation_provisioning` | 2 | canonical encrypted DM pairs and downgrade prevention |
| `v16_e2ee_commit_mailbox` | 2 | recipient-bound Welcomes and pending per-device commit deliveries |
| future attachment migration (number TBD) | 4 | `e2ee_attachment_blobs`, `e2ee_attachment_acks` |
| future guild-channel migration (number TBD) | 6 | `e2ee_channel_membership`, `e2ee_channel_reconciliation` |
| future KT migration (number TBD) | 8 | `e2ee_kt_entries`, `e2ee_kt_checkpoints` |

### Gateway Events (new)

| Event type | Phase | Scope |
|------------|-------|-------|
| `mls_message` | 2 | channel |
| `mls_commit` | 2 | channel |
| `mls_welcome` | 2 | channel |
| `mls_proposal` | 2 | channel |
| `device_list_update` | 1 | user |
| `keypackage_low` | 1 | user |

### REST Endpoints (new)

| Endpoint | Phase |
|----------|-------|
| `PUT /e2ee/devices/{device_id}` | 1 |
| `GET /e2ee/users/{user_id}/devices` | 1 |
| `POST /e2ee/keypackages` | 1 |
| `POST /e2ee/keypackages/claim` | 1 |
| `GET /e2ee/groups/{group_id}/info` | 2 |
| `GET /e2ee/groups/{group_id}/commits` | 2 |
| `POST /e2ee/groups/{group_id}/commits` | 2 |
| `POST /e2ee/groups/{group_id}/commits/ack` | 2 |
| `POST /e2ee/groups/{group_id}/messages` | 2 |

### Desktop Client (Tauri) Changes

| Change | Phase |
|--------|-------|
| E2EE IPC surface (commands + ciphertext in, plaintext + verified state out) | 1 |
| Encryption settings panel | 1 |
| Key isolation boundary (MLS in Rust core, never in JS heap) | 1 |
| Bundled assets enforcement (no remote-loaded UI) | 1 |
| Local encrypted store (SQLCipher) | 1 (foundation), 4 (full) |
| SFrame media encryption in Rust host | 5 |
| Signed update manifests with downgrade protection | 7 |

### AGENTS.md Stop-and-Ask Triggers by Phase

| Trigger | Phases |
|---------|--------|
| Adding new cryptography dependency | 0, 8 |
| Changing protocol event compatibility | 0, 2, 8 |
| Introducing non-Rust SFU alternative | 5 |
| Relaxing limits/timeouts/rate limits | 6 |
| Adding new privileged Tauri APIs | 1 |
| Adding `unsafe` Rust | any |

---

## Subagent Dispatch Template

When dispatching a phase subagent, use this structure:

```
Goal: <phase goal from above>

Context:
- You are implementing Phase <N> of the Filament E2EE implementation plan.
- The full design specification is in plans/PLAN_E2EE.md, section "<section name>".
- Project guidelines are in AGENTS.md — read it before starting.
- Security contracts are in docs/SECURITY.md §"End-to-End Encryption (MLS) Baseline".
- Threat model is in docs/THREAT_MODEL.md §"E2EE Threats".
- Previous phases are committed on this branch; read their code for integration patterns.
- <phase-specific integration points from above>

Deliverables:
<copy the deliverables list from the phase section>

Exit criteria:
<copy the exit criteria checklist from the phase section>

Stop-and-ask triggers:
<copy the stop-and-ask triggers from the phase section>

Quality gate (must pass before commit):
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo audit
cargo deny --config cargo-deny.toml check

Commit with: feat(e2ee): phase <N> — <short description>
```
