# PLAN_E2EE.md

## Objective
Design a security-first end-to-end encryption (E2EE) roadmap for Filament direct messaging and calls that is compatible with:
- non-federated, single-server deployments
- hostile-server assumptions
- existing gateway/REST architecture
- phased rollout without breaking current clients

## Scope and Principles
- In scope:
  - 1:1 DM E2EE (text + attachments metadata envelope)
  - Group DM E2EE (text)
  - Optional encrypted messages/files for guild channels (phased, policy-gated)
  - Voice/video E2EE design path (post-text baseline)
- Out of scope for initial launch:
  - federation key exchange
  - custom cryptographic primitives

Design principles:
- Use vetted protocols/libraries only; no bespoke crypto.
- Forward secrecy and post-compromise recovery where feasible.
- Metadata minimization, not metadata elimination.
- Fail closed on malformed crypto envelopes and key state.
- Default plaintext compatibility: encryption is opt-in and explicit.
- Audience control with safety defaults: clients provide smart audience autosuggestions from authoritative membership, while allowing explicit manual recipient key selection before encryption.

## Baseline Reality (Today)
- Current DMs/guild chat are server-readable.
- Search indexing (Tantivy), moderation workflows, and rich server-side query assume plaintext availability.
- Gateway and REST contracts are typed/versioned and already enforce payload limits.

## Threat Model
### Adversaries
- Malicious or curious server operator.
- Network attacker observing/altering traffic (outside TLS boundary assumptions).
- Compromised client device.
- Malicious user in a DM/group DM.

### Security goals
- Server cannot decrypt message content for E2EE conversations.
- Compromise of long-term identity keys does not expose all past messages (forward secrecy).
- Membership changes in group DMs invalidate old sender keys quickly.
- Clients detect key changes and provide trust signals.

### Non-goals
- Hiding who talks to whom, when, and message size/frequency from server.
- Protecting data on a compromised endpoint after decryption.

## Core Design Choice
Adopt a Signal-family architecture:
- 1:1: X3DH-like authenticated key agreement + Double Ratchet session per device pair.
- Group DM: MLS-style group key schedule or Sender Keys with robust rekey semantics.
- Voice/video: SFrame over media tracks, keyed via the same E2EE group session domain.

Implementation note:
- Pick mature Rust-compatible libraries/protocol bindings with active maintenance and permissive licensing.
- Do not write custom ratchets/key schedules.

## Identity and Device Model
- Each user has:
  - identity keypair (long-lived, per account)
  - signed prekeys + one-time prekeys (rotating)
  - per-device key material (desktop/web/mobile sessions treated as distinct devices)
- Server role:
  - key directory and prekey relay only
  - cannot derive plaintext keys if protocol is implemented correctly
- Client must:
  - pin peer identity keys per conversation
  - display key change warnings
  - support explicit trust verification UX (safety number/device fingerprint)
  - resolve recipient device key bundles from authoritative membership + key directory before encrypting

## Key Management UX (Profile / Client Settings)
- Add a dedicated `Encryption Keys` section in profile/client settings.
- Required fields/actions:
  - current device public key (visible, copyable)
  - current device private key (obscured by default)
  - explicit `Copy Private Key` action gated by a warning modal
  - `Refresh Device Keys` action (client-side key rotation/reset)
- `Refresh Device Keys` behavior (destructive by design):
  - generates new device key material locally
  - publishes new public key bundle
  - invalidates local ability to decrypt previously accessible E2EE messages/files for that device unless separately backed up/restored
  - forces conversation rekey flows as needed
- UX must require explicit typed confirmation for key refresh (for example `REFRESH KEYS`) and show irreversible-impact warnings.

## Audience Resolution Model
- Client builds an initial suggested encryption audience from trusted conversation/channel participant membership:
  - DM 1:1: self + peer devices
  - Group DM: all active participant devices
  - Guild encrypted message/file (if policy allows): members currently authorized for the target channel context
  - Voice/video encrypted session: currently joined participant devices for the media room epoch
- User may manually adjust audience (include/exclude recipients/devices) before send when policy allows.
- Role-based expansion is allowed as a convenience selector, but must resolve to explicit recipient/device sets prior to encryption.
- For guild encrypted messages, client may include the server encryption key as an additional recipient when policy/permissions allow (server-included encrypted mode).
- Clients must not accept sender-provided recipient lists as authority.
- Membership/key snapshot used for encryption must include a version/epoch marker.
- If membership or key state is stale/ambiguous, client must refresh and fail closed rather than send under uncertain audience.

## Conversation Types and Crypto Modes
- `dm_mode = plaintext | e2ee_v1`
- `guild_channel_crypto_policy = plaintext_only | mixed_optional_e2ee` (server-configurable)
- `message_crypto_mode = plaintext | e2ee_v1` (per-message for mixed contexts)
- `guild_encrypted_delivery_mode = client_only | client_plus_server`
- Friends-only DM remains allowed policy for v1.
- E2EE migration policy:
  - new DMs default to `plaintext` unless users explicitly enable E2EE
  - existing conversations can upgrade, never silently downgrade
  - downgrade requires explicit user action + warning
  - encrypted messages/files must carry explicit client-visible encrypted markers
  - guild encrypted messages must carry explicit delivery-mode metadata (`client_only` vs `client_plus_server`)

## Guild Searchable Encryption Mode (Server-Included Recipient)
- Goal: allow encrypted send UX while preserving server-side search/moderation when desired.
- Mechanism:
  - sender client encrypts to intended client audience and also to a server-held public key recipient
  - server can decrypt only messages marked `client_plus_server`
  - `client_only` messages remain opaque to server content inspection
- UX requirement:
  - composer exposes delivery mode selector in guild encrypted-send flow
  - mode must be explicit and visible before send
  - message rows show whether server inclusion was used

## Encryption Send Permissions (Channel Policy)
Add channel-level permissions (or equivalent role permissions) for encrypted messaging:
- `send_encrypted_message`:
  - allows sending encrypted messages in the channel
- `send_encrypted_message_without_server`:
  - allows encrypted send with `client_only` delivery mode (no server recipient)
- Enforcement model:
  - if user has `send_encrypted_message` but not `send_encrypted_message_without_server`, encrypted sends must use `client_plus_server`
  - channels may enforce server inclusion for all encrypted messages via permission configuration
  - clients must fail closed if attempted mode violates effective permissions
  - server must independently enforce the same policy and reject invalid encrypted sends even if client attempts to bypass UI checks
  - required rejection case: `encrypted_delivery_mode=client_only` without `send_encrypted_message_without_server` permission
  - rejection response should be deterministic and typed (for example `403` with explicit error code such as `encrypted_server_inclusion_required`)

## Data Model (Server-Side)
For E2EE conversations, server stores:
- ciphertext payload blob
- envelope metadata:
  - conversation_id
  - sender_user_id, sender_device_id
  - message_id, created_at_unix
  - crypto_suite/version
  - key_epoch / ratchet header fields (bounded)
  - optional attachment descriptor ciphertext references

Server must not store plaintext content for `e2ee_v1` conversations.
For mixed plaintext/E2EE contexts, each message/file record must include explicit crypto mode fields so clients can render trustworthy badges and enforce fail-closed parsing.
For guild encrypted messages, persist explicit delivery-mode metadata and server-inclusion marker for policy/audit correctness.

## Cryptography Options (Initial Selection)
Locked baseline for `e2ee_v1` planning:
- Identity signature keys: `Ed25519`
- ECDH agreement keys: `X25519`
- KDF: `HKDF-SHA-256`
- Symmetric message/file envelope AEAD: `XChaCha20-Poly1305`
- Randomness: CSPRNG from platform cryptographic providers only

Notes:
- No custom primitives or protocol variants beyond vetted library behavior.
- Final crate/library selection still requires ADR and supply-chain review.

## API/Protocol Additions (Design)
### Key directory/prekey endpoints
- `GET /e2ee/keys/me`
- `PUT /e2ee/keys/me/identity`
- `POST /e2ee/keys/me/prekeys/upload`
- `POST /e2ee/keys/claim` (claim recipient prekey bundle)
- `GET /e2ee/keys/{user_id}` (public identity + signed prekey material)

### Conversation capability endpoints
- `POST /dm/conversations/{id}/crypto/enable`
- `GET /dm/conversations/{id}/crypto/state`
- `POST /dm/conversations/{id}/crypto/rekey`

### Message transport
- REST/gateway send routes carry encrypted envelope for E2EE conversations.
- Gateway event examples (new):
  - `dm_e2ee_message_create`
  - `dm_e2ee_rekey`
  - `dm_e2ee_key_update`

All events remain inside `{ v, t, d }` envelope with strict bounds.

### Message/file encrypted markers
- Add explicit wire fields for message/file encryption state:
  - `crypto_mode`
  - `crypto_suite`
  - `key_epoch`
  - optional `sender_device_id`
  - `encrypted_delivery_mode` (`client_only` | `client_plus_server`)
  - `server_recipient_included` (boolean, must match mode semantics)
- Clients must never infer encryption state from heuristics; only trusted typed fields.

### Recipient key fanout
- Sender client encrypts a content key to each recipient device public key (or protocol-equivalent group mechanism) derived from audience resolution.
- Encrypted envelope must include bounded recipient metadata references needed for decryption routing, without exposing plaintext content.
- Server forwards encrypted payloads and cannot alter audience semantics without detectable verification failure at recipients.
- For manual audience mode, client must display unresolved/missing recipient keys before send and block send unless user resolves or explicitly removes those recipients.

## Multi-Device Key Portability
- Add secure cross-device bootstrap flow for user key material:
  - QR-mediated pairing between trusted logged-in devices
  - short-lived out-of-band verification code
  - encrypted key package transfer (never plaintext keys in transit/storage)
- Recovery options remain policy-driven:
  - no-backup strict mode, or
  - passphrase-encrypted backup blob
- Device add/remove events must trigger key state updates and relevant rekey prompts.

## Voice/Video E2EE Direction
Phase after text E2EE stabilization:
- Use SFrame for media frame encryption.
- Key material derived from conversation/group epoch keys.
- Rekey on participant join/leave and periodic rotation.
- SFU (LiveKit) forwards encrypted media; cannot decrypt media payload.

Open question:
- whether chosen web/desktop stack and LiveKit path support required insertable-stream primitives uniformly across targets.

## Product/Moderation/Search Tradeoffs
With E2EE enabled:
- Server-side full-text search on message bodies is unavailable.
- Server-side content moderation/scanning is unavailable for ciphertext.
- Abuse tooling shifts toward:
  - metadata/rate controls
  - client-side user reporting with explicit plaintext disclosure by reporter
  - block/mute and friendship controls

Policy recommendation:
- Keep default guild behavior plaintext for moderation/search viability.
- Introduce guild E2EE as opt-in per message/file only after DM/group E2EE is stable and moderation policy is explicitly documented.
- For guild encrypted mode, allow policy-enforced `client_plus_server` as searchable encrypted delivery where moderation/search requirements are mandatory.

## UX Requirements
- Explicit conversation badge: `End-to-end encrypted`.
- Explicit per-message/per-file badge when encrypted in mixed contexts.
- First-use trust screen explaining guarantees and limits.
- Key-change warnings:
  - passive indicator + blocking interstitial for high-risk changes
- Device management UI:
  - view/remove own devices
  - show peer devices in conversation security panel
  - expose local device key panel in profile/client settings with copy actions
  - private-key copy requires modal warning + explicit user confirmation
  - key refresh action requires destructive-action confirmation
- Backup/recovery UX:
  - encrypted key backup option (passphrase protected) or clearly documented non-recoverability

## Migration and Compatibility
- Mixed-version support:
  - old clients continue plaintext only
  - e2ee messages/files sent only when all required audience participants/devices are e2ee-capable, or blocked with capability error
- No silent plaintext fallback once convo is marked `e2ee_v1`.
- Contract tests ensure unknown/new crypto events are ignored safely by non-upgraded clients.

## Security Controls and Limits
- Strict max sizes on key bundles and encrypted envelopes.
- Per-user/per-route limits for key uploads/claims/rekeys to prevent abuse.
- Replay protection via message counters/nonces per ratchet protocol.
- Audit logs for key directory mutations (no secret material in logs).
- Zero sensitive key material in telemetry/tracing output.

## Dependency and Supply-Chain Gate
Before implementation:
- Select candidate crypto libraries.
- Validate:
  - maintenance activity
  - license compatibility (MIT/Apache/BSD/ISC)
  - external audit status (if available)
- Add ADR documenting final selection and rejected alternatives.

## Rollout Phases
### Phase 0: Design Lock
- Finalize protocol choice (Signal-family specifics).
- Finalize device model and trust UX.
- Finalize downgrade and recovery policy.

Exit criteria:
- ADR approved.
- Threat model and protocol doc updates merged.

### Phase 1: Key Infrastructure
- Implement key directory + prekey endpoints.
- Add client key store abstraction and secure local storage integration.
- Add secure device-to-device key bootstrap flow (QR + verified transfer path).
- Implement profile/client-settings key panel (public/private key visibility/copy flows).
- Implement destructive client-side key refresh flow with forced confirmation UX.
- Add key lifecycle tests and abuse/rate limits.

Exit criteria:
- Deterministic integration tests for key publish/claim/rotation.

### Phase 2: 1:1 DM E2EE (Text)
- Add encrypted message envelope transport for DMs.
- Implement per-device session setup and ratchet message flow.
- Add key-change detection UX.

Exit criteria:
- Two-device and multi-device 1:1 tests pass.
- Server cannot decrypt fixture ciphertext in tests.

### Phase 3: Group DM E2EE (Text)
- Add group key management (MLS or sender-key approach).
- Implement join/leave rekey semantics.

Exit criteria:
- Membership churn tests verify removed members cannot decrypt new messages.

### Phase 4: Mixed-Mode Encryption Markers + File Encryption
- Add per-message/per-file encrypted markers and capability checks in DM/group DM.
- Add encrypted attachment envelope handling and download/decrypt flow.
- Add manual recipient/device picker UX with autosuggested audience and role-expansion controls.

Exit criteria:
- Mixed plaintext/E2EE threads render correct badges with no ambiguous state.
- Encrypted files remain opaque to server content inspection.

### Phase 5: Voice/Video E2EE
- Add SFrame-based media encryption path.
- Rekey on participant changes and interval rotation.

Exit criteria:
- SFU relays encrypted media; decryption only at endpoints.

### Phase 6: Guild Optional E2EE Messages/Files
- Add server policy-gated guild channel mixed-mode E2EE support.
- Add selectable guild encrypted delivery mode (`client_only` vs `client_plus_server`) with explicit metadata.
- Add server-recipient key distribution/rotation path for `client_plus_server`.
- Add channel permission enforcement for:
  - `send_encrypted_message`
  - `send_encrypted_message_without_server`
- Require explicit channel/user capability checks and clear UX warnings.
- Document moderation/search limitations per encrypted post/file.

Exit criteria:
- Guild encrypted posts/files are explicit, opt-in, and fail closed on unsupported clients.
- Permission-gated forced server-inclusion behavior is enforced and tested.
- Server-side rejection path for unauthorized `client_only` encrypted sends is covered by REST and gateway integration tests.

### Phase 7: Hardening and GA
- Pen-test/fuzz pass on envelope parsing and key state handling.
- Load-test key operations and rekey storms.
- Final UX/docs/trust disclosures.

Exit criteria:
- Security review signoff and operational runbooks complete.

## Test Strategy
Server:
- Unit: DTO/newtype validation for crypto endpoints.
- Integration: key upload/claim/rotation/rekey rate limits.
- Negative tests: malformed envelopes, replay attempts, stale epochs.

Client:
- Unit: key state machine transitions.
- Integration: multi-device handshake/ratchet flows.
- UX tests: key-change warnings and downgrade prevention.

Cross-system:
- End-to-end encrypted fixture tests proving server stores/forwards ciphertext only.
- Compatibility tests for mixed client versions.

## Open Decisions
1. Group crypto protocol:
- Option A: MLS-style (strong group semantics, more complexity).
- Option B: sender-key model (simpler, may need stronger operational rekey discipline).
2. Key backup strategy:
- Option A: no backup (strong simplicity, recovery pain).
- Option B: encrypted backup with user passphrase.
3. Voice/video E2EE start timing:
- Option A: after group text E2EE is stable (recommended).
- Option B: parallel with group text work (higher risk).
4. Guild E2EE policy model:
- Option A: per-message/per-file opt-in in policy-enabled channels (recommended).
- Option B: channel-wide mandatory E2EE mode.
5. Server-included encryption key management:
- Option A: one server keypair per workspace (recommended initial simplicity).
- Option B: per-channel server keypairs (stronger isolation, higher operational complexity).

## Immediate Next Slice
- Create an ADR for protocol/library selection and trust UX policy.
- Draft `docs/THREAT_MODEL.md` E2EE section updates.
- Define wire contracts for key endpoints and `dm_e2ee_*` events before coding.
