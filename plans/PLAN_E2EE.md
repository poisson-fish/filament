# PLAN_E2EE.md (v2.1 — MLS baseline)

## Objective
Design a security-first end-to-end encryption (E2EE) roadmap for Filament DMs, group DMs, guild encrypted channels, and calls, built on a single MLS (RFC 9420) stack via OpenMLS, and designed against a hostile server operator with full archive capability. Compatible with:
- non-federated, single-server deployments
- hostile-server assumptions (server = adversary #1, including its ciphertext archive)
- existing gateway/REST architecture
- phased rollout without breaking current clients

## Scope and Principles
- In scope:
  - 1:1 DM E2EE (implemented as 2-member MLS groups)
  - Group DM E2EE (text + attachments)
  - Guild encrypted channels (channel-type, policy-gated; not per-message)
  - Voice/video E2EE via SFrame keyed from MLS exporter secrets
  - Multi-device, device-to-device history sync, local search, mailbox retention model
- Out of scope for initial launch:
  - federation key exchange
  - custom cryptographic primitives or protocol variants
  - web-client E2EE participation (see Identity and Device Model, and Client Architecture)
  - any server-readable "encrypted" delivery mode (key escrow) — removed by design

Design principles:
- One vetted protocol stack (MLS via OpenMLS) for all E2EE domains. No bespoke ratchets, key schedules, or parallel crypto stacks. One audit surface.
- Forward secrecy (FS) and post-compromise security (PCS) are defaults, not aspirations. Design for "a key will leak eventually": a one-time compromise must be neither retroactively catastrophic nor silently persistent.
- The server is a routing and ordering service. Server-provided fields are hints; every security-relevant fact (membership, device lists, encryption state) is verified cryptographically client-side.
- Local-first history: the client's encrypted store is canonical. The server is a delivery mailbox for E2EE payloads, not an archive.
- Fail closed on malformed crypto envelopes, stale epochs, unverifiable state, and capability gaps.
- Encryption is a property of a conversation/channel (invariant), never a per-message toggle.
- No key escrow. Content is either E2EE or honestly plaintext. There is no middle mode.
- Metadata minimization, not metadata elimination (see Non-goals).
- Default plaintext compatibility: encryption is opt-in and explicit; upgrades never silently downgrade.

## Baseline Reality (Today)
- Current DMs/guild chat are server-readable.
- Search indexing (Tantivy), moderation workflows, and rich server-side query assume plaintext availability. For E2EE conversations these move client-side or are explicitly unavailable (see Moderation and Search sections).
- Gateway and REST contracts are typed/versioned and already enforce payload limits.
- As of 2026-07-19, the Phase 0 engineering artifacts and the Phase 1
  identity/device/KeyPackage foundation are implemented. ADR ratification and
  the enforceable cargo-vet gate remain open. No encrypted conversation
  transport or client UI is enabled; see `PLAN_E2EE_IMPL.md` for the exact
  completed/remaining split.

## Threat Model
### Adversaries
- Malicious or curious server operator, including: reading the database, retaining a permanent ciphertext archive, mutating directory/membership data, withholding or reordering delivery, and (for web) serving malicious client code.
- Network attacker observing/altering traffic (outside TLS boundary assumptions).
- Compromised or stolen client device (one-time and ongoing).
- Malicious user or device inside a conversation.

### Security goals
- Server cannot decrypt content for E2EE conversations.
- Forward secrecy: compromise of any key material at time T does not reveal messages sent before T. The operator's ciphertext archive is worthless without per-message keys that no longer exist.
- Post-compromise security: a one-time key/state theft stops working after the victim's next MLS update/commit round trip.
- Membership integrity: the server cannot add or substitute members or devices, and can never expand a group's read audience. Membership changes are member-signed MLS proposals/commits; injected "ghost" users/devices fail signature verification at every client. Server-initiated removals exist only as signed, client-validated external proposals (Remove-only; see Moderation) — they can shrink read access, never grow it.
- Encryption-state integrity: encrypted badges derive only from successful local cryptographic verification, never from server-supplied fields.
- Delivery integrity: withheld/dropped messages are detectable via per-sender MLS generation counters ("messages may be missing" indicator).
- Retention minimization: server retains E2EE ciphertext only transiently (mailbox model), shrinking the harvest-now-decrypt-later surface.

### Non-goals
- Hiding who talks to whom, when, and approximate message frequency from the server (it routes and orders; it necessarily knows group membership for delivery).
- Protecting data on a compromised endpoint after decryption.
- Traffic-analysis resistance beyond size-bucket padding.
- Post-quantum authentication in v1 (PQ confidentiality is roadmapped; PQ signatures are deferred industry-wide because auth attacks cannot be harvested).

### Acknowledged residual risks (documented in trust disclosures)
- First-contact key claims are TOFU until users verify safety numbers or key transparency ships (Phase 8).
- The server can withhold or reorder ciphertext (detectable, not preventable).
- Packaged-client build integrity is a trust dependency; build machines and signing-key custody can be compromised. Mitigated by signed, reproducible builds and downgrade-protected updates (see Supply Chain); signing proves origin, not honesty.
- A compromised webview/renderer inside a packaged client can read plaintext currently displayed, even though key material is isolated in the native core (see Client Architecture).

## Core Protocol Decision (locked for planning; ratify in ADR)
Adopt MLS (RFC 9420) via OpenMLS for every E2EE domain:
- 1:1 DM: a 2-member MLS group.
- Group DM: an MLS group.
- Guild encrypted channel: a large MLS group whose membership tracks channel authorization.
- Voice/video: SFrame media encryption keyed from the MLS `exporter_secret` of the corresponding group epoch.

Rationale (record in ADR):
- Member-signed commits + transcript hash give cryptographic membership agreement: the anti-ghost-user property by construction, which sender-key designs must hand-build.
- O(log N) rekeying makes guild-scale encrypted channels feasible; epochs give crisp add/remove semantics (removal = cryptographic eviction).
- Exporter secrets key SFrame directly (precedent: Discord's DAVE protocol uses MLS for E2EE calls).
- Single-server deployment trivially provides the total ordering of commits that MLS's Delivery Service requires — our architecture's structural advantage.
- One stack for 1:1/group/guild/calls = one dependency, one supply-chain review, one audit surface.
- Rejected alternatives (record with rationale): libsignal (AGPL-3.0, fails license gate; infra-coupled), vodozemac/Olm+Megolm (weaker group PCS/removal discipline, no membership authentication, no exporter analog, no PQ path), static per-device asymmetric (no FS/PCS against an archive-holding adversary).
- Accepted tradeoff: MLS application messages are leaf-signed, hence non-repudiable within the group (unlike Signal's deniable authentication). Record acceptance in ADR.

Cryptographic parameters:
- Baseline ciphersuite: `MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519` (0x0003). X25519 + ChaCha20-Poly1305 + Ed25519, HKDF-SHA-256 — matches our curve/AEAD family preferences with vetted-library behavior only.
- Ciphersuite agility is mandatory in all wire formats and stored state. Adopt a hybrid post-quantum suite (X25519+ML-KEM via HPKE, e.g. X-Wing-style) as soon as standardized and vetted — target harvest-resistance for key establishment ("Level 2") as a fast follow, periodic PQ rekeying ("Level 3") later.
- Framing: all application messages AND commits/proposals are sent as MLS `PrivateMessage`. The server handles opaque blobs plus a minimal routing envelope; it never parses MLS interiors beyond size/shape bounds.
- Randomness: platform CSPRNG only. Key material zeroized on drop (e.g. `zeroize`), held in platform secure storage where available.

## Identity and Device Model
- Per-user root identity key (Ed25519):
  - generated on the user's first device; never leaves devices except via encrypted pairing transfer or opt-in encrypted backup
  - signs device certificates; is the anchor for safety numbers
- Per-device key material:
  - MLS signature keypair + HPKE init keys per device
  - each device holds a device certificate: (user_id, device_id, device signature pubkey) signed by the user root key
  - MLS leaf credentials embed the device certificate; peers verify the chain to the pinned root key
- Server role:
  - directory + KeyPackage relay + Delivery Service (commit ordering) only
  - cannot mint devices: it never holds the root key, so injected devices fail certificate verification at every peer — device injection is cryptographically blocked, not merely policy-blocked
- Device lifecycle:
  - device additions are signed by an existing device (QR pairing flow) and surfaced in-conversation to peers ("X added a new device")
  - device removal is first-class: triggers MLS Remove of that device's leaves from all groups (cryptographic eviction) plus KeyPackage tombstoning
- KeyPackages (MLS's prekey analog):
  - per-device pool of single-use KeyPackages + one ordered, single-use fallback;
    reusable MLS last-resort behavior remains disabled until the corresponding
    extension is implemented and separately reviewed
  - client replenishment on low-water mark; server-side pool size caps, claim rate limits, and claim audit logging
- Client must:
  - pin peer root keys per user; display key-change warnings (passive indicator; blocking interstitial for previously-verified contacts)
  - support safety-number/QR verification of root key fingerprints
  - verify device certificates and membership commits locally; never trust server-asserted device lists or membership
- Web clients:
  - excluded from E2EE in v1. The exclusion is a code-delivery property, not a protocol or JS/DOM limitation: OpenMLS runs in WASM, and MLS ships in comparable runtimes elsewhere (Discord DAVE). But a web page re-fetches its application code from the operator on every load, so a hostile operator can serve a targeted, key-exfiltrating build to a single user on a single load and revert without trace — incompatible with adversary #1. This hole applies to any E2EE protocol executed in browser-delivered code.
  - E2EE participation requires packaged desktop/mobile builds (signed, reproducible; see Client Architecture and Supply Chain)
  - web UX for E2EE conversations is a fail-closed capability state: conversation existence may render (the server necessarily knows membership for routing), content renders as "end-to-end encrypted — open in a packaged client". No plaintext fallback exists, and no server-side decryption path exists to fall back to.
  - a logged-in web session of the same user has no decryption capability: keys are device-bound, never account-bound
  - revisit only as an explicitly disclosed degraded trust tier (Open Decisions)

## Client Architecture (Packaged Clients)
The unified SolidJS frontend is retained across web, desktop, and (later) mobile. E2EE changes where code and keys live, not the UI stack.

- Desktop (Tauri + SolidJS):
  - UI assets are bundled inside the signed package and served from the local application protocol. Remote-loading the hosted web UI into the shell is prohibited: a webview pointed at server-delivered code is a browser with extra steps and inherits the web trust model wholesale.
  - Crypto core placement: OpenMLS and all key/state operations run in the Rust host process — not as WASM inside the webview. The webview communicates with the core over a narrow, typed IPC surface: commands and ciphertext in, plaintext and verified state out.
  - Key material never enters the JS heap. This shrinks the blast radius of any webview compromise (XSS, renderer bug, content-safety bypass) from "steal keys" to "read plaintext currently on screen."
- Mobile (aligns with main-plan Phase 9):
  - same shared Rust core over FFI (Swift/Kotlin bindings); platform keystores for custody; the same narrow-boundary discipline between UI and core.
- Device-bound keys:
  - decryption capability follows paired, certified devices — never account credentials. Account login in a non-capable client (e.g. the web app) confers nothing.
- Update integrity:
  - signed update manifests with downgrade protection; reproducible builds and binary transparency per Supply Chain. Code signing converts the web-model attack (silent, targeted, per-load substitution) into a release-pipeline compromise that ships an auditable artifact to every user — a necessary floor, not the full answer.

## Key Management UX (Profile / Client Settings)
- `Encryption` settings panel:
  - user safety number / root key fingerprint (shareable, QR)
  - device list: name, added date, verification state, `Remove device` action
  - `Rotate identity` action (destructive, typed confirmation e.g. `ROTATE IDENTITY`)
  - backup enrollment status and controls
- No private-key display or copy action exists anywhere in the product. Key material leaves a device only via (a) QR-mediated encrypted device pairing or (b) opt-in passphrase-encrypted backup. Keys are non-exportable and live in platform keystores (Keychain / Android Keystore / TPM+DPAPI) where available.
- `Rotate identity` semantics (documented honestly in UX):
  - generates a new root key, recertifies devices, rejoins groups; peers receive blocking key-change warnings
  - does NOT delete local history and CANNOT protect already-sent ciphertext (forward secrecy already handles the past; nothing can retroactively re-protect it)
  - value: identity continuity reset after suspected compromise
- Lost/stolen device flow: `Remove device` from any remaining device evicts it from all groups at the next commit; if no device remains, account recovery = new identity (or backup restore) with peer re-verification.

## History, Storage, and Retention
Forward secrecy makes server-stored ciphertext one-shot: message keys are deleted after use, so clients can never re-decrypt old server blobs. Consequences, designed in rather than discovered later:
- Local encrypted message store per device (SQLCipher-or-equivalent; store key in platform keystore) is the canonical history.
- Server storage for E2EE payloads is a mailbox: retain ciphertext until all member devices acknowledge delivery or a TTL expires (configurable, e.g. 30 days), then hard-delete. No long-term ciphertext archive by default — this directly shrinks the operator's harvest surface and is a headline security property.
- New-device history: device-to-device encrypted history sync from an existing device (or backup restore). QR pairing alone grants future messages only.
- Backup (opt-in): passphrase-encrypted blob (Argon2id at aggressive parameters) covering identity keys + history snapshot; clearly documented non-recoverability if declined.
- Disappearing messages: per-conversation timer, negotiated inside ciphertext, enforced client-side, mirrored by server mailbox TTL.
- Local search: client-side full-text index built from the local plaintext store (SQLite FTS5 on mobile; Tantivy in the desktop client), encrypted at rest. This replaces server-side Tantivy for E2EE conversations.

## Conversation Types and Crypto Modes
- `conversation_crypto = plaintext | mls_v1` — a property of the conversation/channel, immutable except via explicit upgrade.
- 1:1 DM and group DM: user-explicit enable at creation or upgrade. Upgrade creates the MLS group and marks the conversation; no silent downgrade ever; "downgrade" = explicitly creating a new plaintext conversation.
- Guild channels: `channel_type = plaintext | encrypted`, set at channel creation, permission-gated. The entire channel is one mode. No per-message crypto toggles, no mixed channels, no composer mode selector. (Per-message mixing invites users to lose track of which mode they are typing in; channel-invariant modes match the Signal-style mental model.)
- The server-included recipient / `client_plus_server` searchable-encryption mode is REMOVED. Rationale: under our threat model it is key escrow — zero confidentiality against adversary #1 while rendering a lock icon users will misread as privacy. Channels that require server-side moderation/search remain honestly plaintext.
- Capability gating: an E2EE conversation requires every participant to have at least one MLS-capable device; otherwise block with a typed capability error. Fail closed; no plaintext fallback. Gating cuts both ways: a participant whose only client is the web app is not MLS-capable, so their presence blocks creation/upgrade with the typed error rather than silently degrading the conversation.
- Friends-only DM policy remains allowed and orthogonal.

## Audience Model
- Audience = MLS group membership. Nothing else. No per-message recipient editing, no per-send role-expansion selectors, no manual device picking. (Per-message audience editing is incompatible with MLS group semantics and created undecryptable-ghost and epoch-race states; a secret subset is simply a different conversation.)
- Membership changes occur via member-signed MLS proposals/commits. One constrained exception: the server is registered as an MLS external sender permitted to propose removals only (ban/kick/role-loss enforcement — see Moderation); commits remain member-signed, and clients hard-reject any externally-proposed Add. The server relays and orders; it can never expand a group's read audience.
- Guild encrypted channels: authorization-to-membership reconciliation
  - joining a channel = an Add proposal committed by an authorized member/admin device per channel permissions
  - leave/kick/role-loss = a Remove commit; permission changes reconcile to commits promptly (bounded reconciliation window, monitored)
  - the server enforces WHO MAY PROPOSE (policy); clients enforce WHAT IS CRYPTOGRAPHICALLY VALID (signatures, epoch state) — both must pass
- Stale/ambiguous state: if group state is behind or conflicted, client refreshes, rebases, and fails closed rather than sending under uncertainty.

## Data Model (Server-Side)
For `mls_v1` conversations, server stores:
- opaque MLS ciphertext blob (`PrivateMessage`)
- minimal routing envelope:
  - conversation_id / group_id
  - message_id, created_at_unix
  - epoch tag (for Delivery Service ordering) and suite id
  - bounded sizes; contents padded client-side to size buckets (e.g. 512 B / 1 KiB / 4 KiB / 16 KiB) to blunt size fingerprinting
- per-device delivery acknowledgments (drives mailbox GC)
- KeyPackage pools and device certificates (public material only)
- optionally, the group's current encrypted GroupInfo/ratchet-tree blob published by members to support joins and external-commit recovery (treated as sensitive-not-secret: it encodes membership structure, which the server already learns from routing)

Server must not store plaintext content, content-derived metadata, or unwrapped key material for `mls_v1` conversations. There are no mixed-mode records: a conversation's records are uniformly plaintext or uniformly MLS blobs, and clients fail closed on any record whose local verification contradicts the conversation's pinned mode.

## API/Protocol Additions (Design)
### Identity, device, and KeyPackage endpoints
- `PUT /e2ee/devices/{device_id}` — publish device certificate (root-key-signed)
- `GET /e2ee/users/{user_id}/devices` — certified device list (clients verify signatures; list is a hint, certificates are the truth)
- `POST /e2ee/keypackages` — upload KeyPackage pool for a device
- `POST /e2ee/keypackages/claim` — claim a KeyPackage for a target user/device (rate-limited, audited)
- `GET /e2ee/groups/{group_id}/info` — encrypted GroupInfo for joins/recovery, where group policy allows

### Group and message transport
- `POST /e2ee/groups/{group_id}/commits` — Delivery Service ingestion point; enforces total order per group
  - single-writer-per-epoch: the first order-valid commit for epoch N is accepted; competing commits receive a deterministic typed rejection (`409 epoch_conflict`) and clients rebase pending proposals
- `POST /e2ee/groups/{group_id}/messages` — application `PrivateMessage` transport
- Gateway events (new), all inside the `{ v, t, d }` envelope with strict bounds:
  - `mls_message`, `mls_commit`, `mls_welcome`, `mls_proposal`
  - `device_list_update`, `keypackage_low`
- Wire fields on message records: `crypto` (plaintext|mls_v1), `suite`, `epoch`, `sender_device_id` — routing hints only. Clients derive all trust state (badges, sender identity, membership) from local MLS verification against pinned group state; a server field can never upgrade a message's displayed trust.
- Server-side validation is shape-only for MLS payloads: size bounds, field presence, epoch monotonicity per group. The server never parses MLS interiors.

## Attachments (E2EE conversations)
- Random per-file content key; AEAD encryption aligned with the group suite; no convergent encryption or cross-user deduplication (equality oracle).
- File key + metadata (filename, MIME, size, content hash, thumbnail key) travel inside the MLS application message; the server-side blob and its descriptor are opaque.
- Client-generated encrypted thumbnails; no server-side thumbnailing, transcoding, or unfurling for E2EE content.
- Blob storage follows the mailbox model: padded to size buckets, deleted after all-device fetch or conversation TTL.

## Message-Adjacent Features (E2EE contexts)
Everything message-adjacent rides inside the ciphertext or is disabled:
- Reactions, edits, delete-for-everyone, replies/quote previews, pins: MLS application messages; the server never learns their semantics.
- Link previews: client-generated only, per-conversation opt-in, off by default (server unfurling would exfiltrate URLs and content).
- Read receipts and typing indicators: off by default in E2EE conversations (metadata cost); if shipped, carried inside ciphertext.
- Push notifications: data-only pushes (wake signal + conversation hint at most); notification text is decrypted on-device (iOS notification service extension / Android equivalent). The push pipeline must never carry plaintext.
- Delivery-gap detection: per-sender MLS generation counters drive a "messages may be missing" indicator when gaps persist past a threshold — the honest answer to a server that withholds ciphertext.

## Moderation, Abuse, and Reporting
E2EE removes exactly one moderation tool: covert server-side content inspection. Every other layer survives, and membership enforcement gets stronger than plaintext. Moderation tooling assumes the operator is honestly enforcing workspace policy (it is the owner's own tool); the cryptography exists so that even a dishonest operator can never expand read access — a dishonest operator refusing to enforce policy is a denial-of-service problem, not a confidentiality one.

### Layer 1 — structural moderation (server-controlled, unchanged)
- Workspace policy decides whether encrypted channels exist at all: `encrypted_channel_policy = disabled | require_moderator_membership | unrestricted` (per workspace, optionally per category).
- Channel create/join permissions, invite controls, slowmode/rate limits, channel freeze/delete, workspace-level bans and account actions: all enforced server-side, no plaintext required.

### Layer 2 — membership moderation (policy + cryptography; stronger than plaintext)
- Kick/ban/role-loss = Remove commit = cryptographic eviction: post-removal epochs are mathematically unreadable to the removed member. Even a colluding server has nothing to hand them; plaintext channels only ever offer the server's ongoing promise to keep saying no.
- Enforcement is two-layer: policy acts at t=0 (server stops routing to/from the banned member); crypto lands at the next commit (epoch eviction).
- Server-initiated removals: the server is an MLS external sender authorized to submit Remove proposals only; online member clients validate and auto-commit policy-consistent removals, so eviction does not wait for a moderator to be present. Safe by asymmetry: removals can only shrink read access; abuse of them is griefing the server could already do by dropping delivery. Clients hard-reject externally-proposed Adds — adds are where ghost members live.
- Removal latency is bounded client-side: if a required Remove commit is not ordered within a bounded window, clients block sends in that group and surface a warning (fail closed against a stalling Delivery Service).

### Layer 3 — content moderation (member-based, visible)
- Moderators who are members see plaintext like any member and can warn, delete, and kick. `require_moderator_membership` makes this a channel invariant: the mod-team role is present in every encrypted channel, visible in the member list, disclosed in the channel header.
- This is the honest successor to the removed escrow mode: same human-moderation capability, but the reader is a visible, accountable member device — not a silent key beside the message database. The E2EE claim stays literally true (only members can read); the membership is the disclosure.
- Moderator deletion = mailbox purge of undelivered copies + signed moderation tombstone (honest clients delete locally). Copies already on malicious clients are unreachable — the same physics as screenshots, plaintext or encrypted.
- Automated moderation in encrypted channels, where a workspace wants it, is a visible bot member — never a hidden recipient. Tradeoff stated plainly: a scanning bot re-centralizes plaintext at its host, and if that host is the message server itself, this rebuilds escrow with visibility. Allow/disallow is an Open Decision.

### Layer 4 — report-based moderation
- User reporting with explicit reporter-side plaintext disclosure: a report packages the reporter's decrypted copies plus envelope references; UX makes the disclosure explicit.
- Roadmap (Open Decisions): message franking / committing AEAD so reported content is cryptographically verifiable as genuine.

Policy stance:
- Default guild behavior remains plaintext for full moderation/search viability; compliance- or archive-bound workspaces simply do not enable encrypted channel types.
- Genuinely unavailable in encrypted channels, by design: silent content scanning and retroactive server-side content search where no moderator is a member. Any mechanism granting the server read access "for moderation" grants a hostile operator the same access — keys encode capabilities, not intentions.

## Voice/Video E2EE Direction
- SFrame over insertable streams; SFU (LiveKit) forwards opaque encrypted frames and cannot decrypt media.
- Keys derived from the corresponding MLS group's `exporter_secret`; media epoch == MLS epoch.
- Rekey on participant join/leave (membership commit) and periodic update commits.
- Precedent: Discord's DAVE protocol ships MLS-keyed E2EE calls in this exact product category.
- Webview verification matrix (Phase 5 gate): insertable-streams / `RTCRtpScriptTransform` support must be verified per target — WebView2 (Chromium; expected supported), WKWebView (macOS/iOS), WebKitGTK (Linux) — before media E2EE ships on that platform. Where a webview lacks support, the required fallback is a native WebRTC media path in the host layer; shipping unencrypted media is never the fallback.

## Security Controls and Limits
- Strict max sizes on KeyPackages, commits, Welcomes, proposals, and message envelopes.
- Per-user/per-device/per-route rate limits on KeyPackage uploads/claims, commits, and rekeys; commit-storm backpressure.
- Replay and reorder protection via MLS epoch + per-sender generation counters; server enforces epoch monotonicity, clients enforce everything else.
- Audit logs for directory mutations (device certs, KeyPackage pools) — public material only, never secret material.
- Zero key material in logs, telemetry, tracing, or crash dumps; memory zeroization on drop; secrets in platform keystores.
- Fail closed on: malformed envelopes, unverifiable commits, stale epochs, capability gaps, and any server-field/local-verification mismatch.

## Supply Chain and Build Integrity
- Dependency gate before implementation: `cargo audit` + `cargo vet` (or equivalent), pinned and hash-locked dependencies, license compatibility (MIT/Apache/BSD/ISC — OpenMLS is MIT), external audit status review.
- ADR documents final selection and rejected alternatives with rationale (libsignal: AGPL-3.0 + infra coupling; vodozemac: group semantics, no membership authentication, no exporter, no PQ path; static per-device keys: no FS/PCS against an archive-holding adversary).
- Client build integrity is part of the E2EE trust story: signed releases, reproducible builds as a goal, update-channel integrity (signed manifests, downgrade protection), binary transparency for client releases on the roadmap.
- Code signing is necessary but not sufficient: it proves origin, not honesty. It converts per-user, per-load targeted substitution into an all-users release-pipeline compromise producing an auditable artifact; build machines and signing-key custody remain disclosed trust dependencies. Reproducible builds exist so third parties can verify signed binaries match public source; binary transparency is the endpoint.

## Key Transparency (roadmap, Phase 8)
- Append-only, auditable log of the key directory (root keys, device certificates) with inclusion and consistency proofs; client-side auditing; out-of-band checkpoint distribution.
- Converts "the server can lie silently" (serving different key sets to different users, suppressing rotations) into "the server can lie once and get caught." Precedent: WhatsApp Key Transparency, Apple Contact Key Verification.
- Until KT ships, first contact is TOFU + pinning + safety-number verification, and the trust disclosures say so plainly.

## Rollout Phases
### Phase 0: Design Lock
- ADR: OpenMLS, ciphersuite 0x0003 + agility, deniability acceptance, web-client exclusion (code-delivery rationale), packaged-client architecture (bundled assets, Rust-core crypto behind IPC, device-bound keys), mailbox retention model, backup policy, rejected alternatives.
- Threat model and protocol docs merged; wire contracts for all endpoints/events drafted.
Exit criteria:
- ADR approved; `docs/THREAT_MODEL.md` E2EE section merged.

### Phase 1: Identity, Devices, KeyPackages
- Root identity key generation; device certificates; platform keystore integration.
- QR device pairing with encrypted key transfer; device add/remove flows with in-conversation surfacing.
- KeyPackage pool upload/claim/replenish + ordered, one-time fallback semantics;
  rate limits and claim auditing. Reusable last-resort behavior requires a
  separately reviewed MLS extension.
- Encryption settings panel (safety number, device list, rotate identity, backup enrollment). No key-export surface.
- Local encrypted store foundation (SQLCipher-or-equivalent).
Exit criteria:
- Deterministic integration tests for device publish, KeyPackage claim, rotation, and pairing.
- Negative test: server-forged device certificate is rejected by clients (ghost-device injection fails).
- Key-isolation audit: MLS key material is confined to the Rust core; the webview context has no key access path (IPC surface review plus negative test).

### Phase 2: 1:1 DM E2EE (2-member MLS groups)
- Conversation create/upgrade flow; `PrivateMessage` transport; commit pipeline with epoch-conflict rebase.
- Key-change warnings; gap indicators; mailbox acks and GC.
Exit criteria:
- Two-device and multi-device 1:1 churn tests pass, including out-of-order and offline catch-up.
- Persistence audit: server records for E2EE fixtures contain opaque envelopes only (protocol-level "server cannot decrypt" assurance comes from design review; the test evidences the storage contract).

### Phase 3: Group DM E2EE
- Membership proposals/commits; join via Welcome; removal eviction; external-commit recovery from desync.
Exit criteria:
- Membership churn tests: removed members fail on all post-removal epochs; concurrent commit races resolve deterministically; desync self-heals via external commit.

### Phase 4: Attachments, History, Search
- Encrypted attachment envelopes and download/decrypt flow; encrypted thumbnails.
- Device-to-device history sync; opt-in passphrase backup (Argon2id); disappearing messages; local search index.
Exit criteria:
- New-device onboarding restores history without any server-side plaintext; encrypted files remain opaque to server inspection; mailbox GC verified.

### Phase 5: Voice/Video E2EE
- SFrame keyed from exporter secrets; rekey on membership commits and interval updates; LiveKit opaque-forwarding path.
Exit criteria:
- SFU relays encrypted media only; decryption exclusively at endpoints; join/leave rekey verified.
- Insertable-streams verification matrix complete (WebView2 / WKWebView / WebKitGTK); native media path exercised on any platform lacking webview support.

### Phase 6: Guild Encrypted Channels
- `channel_type = encrypted` with permissioned Add/Remove commit flows reconciling channel authorization to group membership.
- Large-group performance work (tree operations at 10^3–10^4 leaves); capability gating; moderation-limits documentation.
Exit criteria:
- Role-loss eviction tests pass within the reconciliation window; performance budget met at target channel size; encrypted channels fail closed on unsupported clients.

### Phase 7: Hardening and GA
- Fuzzing on MLS ingestion (envelope parsing, commit/state handling); commit-storm and KeyPackage-exhaustion load tests.
- Pen test including adversarial review of the QR pairing protocol; final UX/docs/trust disclosures.
Exit criteria:
- External security review signoff; operational runbooks complete.

### Phase 8: Key Transparency and PQ
- KT log + client auditing; hybrid X25519+ML-KEM ciphersuite adoption once standardized and vetted.
Exit criteria:
- Equivocation detection demonstrated in tests; suite migration exercised on fixture groups.

## Test Strategy
Server:
- Unit: DTO/newtype validation for all `e2ee` endpoints; shape-only MLS payload bounds.
- Integration: KeyPackage upload/claim/replenish under rate limits; commit ordering and `epoch_conflict` determinism; mailbox ack/GC; retention TTLs.
- Negative: malformed envelopes, replayed generations, stale epochs, oversized payloads, unauthorized proposal attempts.

Client:
- Unit: group state machine transitions; device certificate verification; badge derivation from local verification only.
- Integration: multi-device pairing and history sync; join/leave/remove flows; external-commit recovery; RFC 9420 test vectors.
- Isolation: IPC key-isolation tests — the webview context cannot reach key material or invoke raw key operations; crypto is reachable only via the typed IPC surface.
- Adversarial: server-forged device lists and membership commits are rejected; server flipping `crypto` hints triggers fail-closed, never fallback; downgrade attempts surface warnings.
- UX: key-change interstitials, capability errors, disappearing-message enforcement.

Cross-system:
- End-to-end fixtures proving the server stores/forwards opaque blobs only, GroupInfo included.
- Mixed client-version compatibility: old clients remain plaintext-only and safely ignore `mls_*` events (contract tests).
- Padding-bucket verification and metadata-minimization checks on the wire.

## Open Decisions
1. Backup default:
   - Option A: opt-in passphrase backup (recommended — hard no-backup kills consumer adoption; Signal itself relented).
   - Option B: strict no-backup mode as an additional per-user choice.
2. Deniability stance:
   - Option A: accept MLS in-group non-repudiation (recommended; record in ADR).
   - Option B: pursue deniable-authentication variants (rejected complexity for v1).
3. Message franking / verifiable abuse reports: v1.5 candidate — decide after Phase 3 telemetry on report volume.
4. Guild encrypted channel size ceiling: initial hard cap (e.g. 1–5k leaves) vs. uncapped with perf gates.
5. Web client future: permanent exclusion vs. an explicitly disclosed degraded tier. Realistic revisit paths: a Code-Verify-style build-integrity extension, or Isolated Web Apps / signed web bundles if they standardize beyond Chromium — both effectively reinvent packaged distribution, and browser key storage remains weaker than OS keystores. Any web tier would ship as disclosed-degraded; parity is off the table.
6. Baseline suite: 0x0003 (X25519/ChaCha20-Poly1305, recommended for library maturity and performance) vs. 0x0006 (X448, larger margin, weaker ecosystem support).
7. Read receipts / typing indicators in E2EE conversations: ship-at-all decision.
8. KT construction: CT-style static log vs. VRF-based (CONIKS-style) directory; timing relative to GA.
9. Automated moderation bots as visible members of encrypted channels: allow (workspace opt-in, disclosed in member list and channel header) vs. disallow (human moderators only). If allowed, define bot-hosting custody requirements (plaintext access must not co-reside with the message server).

## Immediate Next Slice
- Write the ADR: OpenMLS + suite 0x0003 + agility plan, rejected alternatives with rationale, deniability acceptance, web exclusion (code-delivery rationale), packaged-client architecture (bundled assets, Rust-core crypto behind typed IPC, device-bound keys), mailbox retention model, backup policy.
- Draft `docs/THREAT_MODEL.md` E2EE section updates from this document's threat model.
- Define wire contracts for device/KeyPackage/group endpoints and `mls_*` gateway events before coding.
- Engineering spike: OpenMLS 2-member group round trip (create, add, message, remove, external-commit recovery) in a Rust CLI harness against a fixture Delivery Service.
- Engineering spike: verify insertable-streams / `RTCRtpScriptTransform` availability in WKWebView and WebKitGTK (early; informs the Phase 5 media path and whether a native WebRTC fallback is needed per platform).

## Appendix: Revision Notes (v2.1)
Clarified and added, from client-architecture review:
- Web exclusion rationale made explicit: a code-delivery property (the operator ships the application code on every load and can serve a targeted, single-load key-exfiltrating build), not a protocol or JS/DOM limitation. Applies to any E2EE protocol executed in browser-delivered code.
- New Client Architecture section: the unified SolidJS UI is retained; packaged clients must bundle assets and serve them from the local application protocol (no remote-loaded UI in shells); OpenMLS runs in the Rust host process behind a narrow, typed IPC surface so key material never enters the JS heap; mobile reuses the shared Rust core over FFI; keys are device-bound, never account-bound.
- Web UX contract for E2EE conversations: fail-closed capability state ("open in a packaged client"); no plaintext fallback path exists, and no server-side decryption path exists to fall back to.
- Capability gating clarified as bidirectional: web-only participants block `mls_v1` creation/upgrade with a typed error rather than degrading the conversation.
- Signing framed as necessary-but-not-sufficient (proves origin, not honesty); reproducible builds and binary transparency positioned as the third-party verification path; release-pipeline compromise and webview plaintext exposure recorded as disclosed residual risks.
- Voice/video open question converted into a Phase 5 verification matrix (WebView2 / WKWebView / WebKitGTK) with a native-WebRTC fallback requirement; added an early spike to the next slice.
- Phase 1 exit criteria gained a key-isolation audit; test strategy gained IPC key-isolation tests.
- Open Decision 5 refined with concrete revisit paths (Code-Verify-style integrity extension; Isolated Web Apps / signed web bundles) and a disclosed-degraded ceiling.

## Appendix: Revision Notes (v2)
Removed, with rationale — for the security-stance review session:
- Copyable private key panel: no serious E2EE product exposes raw private keys; it creates a phishing target, forces keys to be extractable (precluding platform keystores), and misrepresents ratchet key semantics. Migration/recovery needs are covered by QR pairing and passphrase backup.
- `client_plus_server` / server-included recipient mode and its permission pair: key escrow under our threat model; zero confidentiality against adversary #1 while rendering a misleading lock icon. Channels needing moderation/search stay honestly plaintext.
- Per-message crypto toggles and mixed channels: replaced by channel/conversation-invariant modes to eliminate composer-mode confusion and ambiguous badge states.
- Per-message manual audience editing and role-expansion send selectors: incompatible with MLS group semantics; a secret subset is a new conversation.
- X3DH-shaped prekey endpoints and the XChaCha20-Poly1305 primitive lock: superseded by MLS KeyPackages and ciphersuite 0x0003 with suite agility.
Changed:
- Signal-family dual-track (Double Ratchet + separate group scheme) → single OpenMLS stack for 1:1, groups, guild channels, and calls.
- "Refresh Device Keys" → "Rotate identity" with honest semantics (identity reset + peer warnings; not history protection or destruction).
- Server-readable-forever ciphertext archive → mailbox retention model with local-first encrypted history and device-to-device sync.
- Server-asserted encryption markers → badges derived exclusively from local cryptographic verification.
Added:
- Root-key-certified device model (ghost-device defense), delivery-gap indicators, size-bucket padding, data-only push, client-side link previews, local search index, disappearing messages, reproducible/signed builds, key transparency and hybrid-PQ roadmap phases.
