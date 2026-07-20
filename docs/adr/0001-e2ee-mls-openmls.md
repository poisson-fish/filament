# ADR 0001: E2EE Protocol Stack — MLS via OpenMLS

- **Status:** Proposed (pending maintainer ratification)
- **Date:** 2026-07-12
- **Deciders:** Filament maintainers
- **Supersedes:** None
- **Binding for:** All E2EE implementation phases (Phase 0 onward)
- **References:** [PLAN_E2EE.md](../../plans/PLAN_E2EE.md) (v2.1), [THREAT_MODEL.md](../THREAT_MODEL.md), [SECURITY.md](../SECURITY.md)

---

## Context

Filament is a security-first, self-hosted Discord-like application. Phases 0–8 of the original [PLAN.md](../../plans/PLAN.md) are complete: authentication, gateway, Postgres persistence, Tantivy search, roles/permissions, LiveKit voice/video, desktop hardening, and deployment/ops. The product now needs end-to-end encryption (E2EE) for DMs, group DMs, guild encrypted channels, and calls.

### Threat Model Driver

The primary E2EE adversary is **the server operator itself** (adversary #1). A hostile operator can:

- Read the database and stored blobs directly.
- Retain a permanent ciphertext archive for future decryption.
- Mutate directory/membership data to inject ghost users or devices.
- Withhold, reorder, or selectively deliver ciphertext.
- For web clients: serve a targeted, key-exfiltrating client build to a single user on a single page load, then revert without trace.

Any E2EE design must treat the server as untrusted for content confidentiality. The server is a routing and ordering service — it must never hold keys that grant content access.

### Why a Protocol Standard

Bespoke ratchets and hand-rolled key schedules are categorically rejected (AGENTS.md §0: "Never implement crypto by hand"). We need a vetted, standardized protocol with:

- Forward secrecy (FS) and post-compromise security (PCS) as defaults.
- Cryptographic membership agreement (anti-ghost-user by construction).
- O(log N) group rekeying for guild-scale encrypted channels.
- An exporter secret mechanism to key media encryption (SFrame).
- A clear path to post-quantum key establishment.

---

## Decision

Adopt **MLS (Messaging Layer Security, RFC 9420)** via the **OpenMLS** Rust library as the single E2EE protocol stack for all encryption domains:

| Domain | MLS Group Shape |
|---|---|
| 1:1 DM | 2-member MLS group |
| Group DM | N-member MLS group |
| Guild encrypted channel | Large MLS group tracking channel authorization |
| Voice/video (calls) | SFrame media encryption keyed from the MLS `exporter_secret` of the corresponding group epoch |

**One stack, one dependency, one supply-chain review, one audit surface.**

### Ciphersuite

Baseline: **MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519** (0x0003).

- X25519 KEM + ChaCha20-Poly1305 AEAD + Ed25519 signatures + HKDF-SHA-256.
- Matches the project's curve/AEAD family preferences.
- Selected for library maturity and performance in OpenMLS.

**Ciphersuite agility is mandatory** in all wire formats and stored state. A hybrid post-quantum suite (X25519+ML-KEM via HPKE, X-Wing-style) will be adopted as soon as it is standardized and vetted — target harvest-resistance for key establishment ("Level 2") as a fast follow, with periodic PQ rekeying ("Level 3") later.

### OpenMLS Dependency

- **Library:** [openmls](https://github.com/openmls/openmls) — MIT licensed. Its `hpke-rs` provider dependencies are MPL-2.0 and use exact-version `cargo-deny` exceptions with binary-distribution controls.
- **Versions:** openmls 0.8.1, openmls_traits 0.5.0, openmls_rust_crypto 0.5.1, openmls_basic_credential 0.5.0, openmls_memory_storage 0.5.0.
- **Crypto provider:** `openmls_rust_crypto` (default) uses RustCrypto crates — no C dependencies, no `unsafe` in the crypto path.
- **Supply chain:** the scoped license gate passes; the advisory gate remains blocked on the published HPKE/libcrux chain. See the supply-chain section below.

### Application Message Framing

All application messages and commits/proposals are transported as MLS `PrivateMessage`. The server handles opaque blobs plus a minimal routing envelope (conversation_id, message_id, epoch, suite, sender_device_id, created_at). The server never parses MLS interiors beyond size/shape bounds.

### Randomness and Key Material

- Platform CSPRNG only.
- Key material is zeroized on drop (`zeroize` crate).
- Keys are held in platform secure storage (Keychain / Android Keystore / TPM+DPAPI) where available.
- No private-key display or copy surface exists anywhere in the product.

---

## Rejected Alternatives

### libsignal (Signal Protocol)

**Rejected.** Reasons:

1. **License incompatibility:** libsignal is AGPL-3.0, which fails the project's license gate (MIT/Apache/BSD/ISC only). AGPL-3.0's network-use disclosure obligation is incompatible with a self-hosted product that does not wish to expose source on every deployment.
2. **Infrastructure coupling:** Signal's protocol is designed for Signal's infrastructure (Sequoia PQR key distribution, sealed sender relay). Adapting it to a different server architecture requires significant rework and introduces bespoke integration risk.
3. **No group exporter:** Signal's group ratchet does not provide an exporter secret analog for keying SFrame media encryption. A separate key schedule would be needed for calls, violating the "one stack" principle.

### vodozemac / Olm + Megolm (Matrix)

**Rejected.** Reasons:

1. **Weaker group PCS and removal discipline:** Megolm's group ratchet uses a shared chain key distributed to all members; a removed member who retained the chain key can decrypt messages sent before the next ratchet advance. MLS's per-member leaf nodes and tree-based rekeying provide cryptographic eviction at the epoch boundary — post-removal epochs are mathematically unreadable.
2. **No membership authentication:** Megolm messages are signed by the sender's session key, not by an identity root. There is no cryptographic chain from a message to a user's verified identity. MLS leaf credentials embed device certificates that chain to pinned root keys.
3. **No exporter analog:** Like libsignal, Megolm has no exporter secret mechanism for deriving SFrame keys from the group state.
4. **No PQ path:** vodozemac has no post-quantum key establishment roadmap. MLS's ciphersuite agility provides a clear path to hybrid PQ.

### Static Per-Device Asymmetric Encryption

**Rejected.** Reasons:

1. **No forward secrecy:** A static per-device keypair provides no FS. If the private key is compromised, all ciphertext encrypted to that key — past and future — is decryptable.
2. **No post-compromise security:** A one-time key theft is permanent until manual rotation. MLS update commits automatically heal after one round trip.
3. **No defense against archive-holding adversary:** The hostile-server threat model explicitly includes a permanent ciphertext archive. Static keys make this archive permanently valuable. FS (per-message keys deleted after use) makes the archive worthless.

---

## Accepted Tradeoffs

### MLS In-Group Non-Repudiation (Open Decision 2)

MLS application messages are leaf-signed by the sender's signature key. Within a group, a message is cryptographically attributable to the sender's leaf node — and through the credential chain, to the user's root identity. This is **non-repudiable authentication within the group**.

This differs from Signal's deniable authentication (X3DH + Double Ratchet provides authentication that is verifiable but not provable to third parties). Pursuing deniable-authentication variants on top of MLS would add significant complexity for v1 with marginal benefit.

**Decision:** Accept MLS in-group non-repudiation. Record this as a disclosed property. Deniable authentication is rejected complexity for v1.

### Guild Encrypted Channel Size Ceiling (Open Decision 4)

Initial hard cap: **5,000 leaves** per encrypted guild channel. Uncapped operation is deferred until performance gates prove tree operations at 10³–10⁴ leaves are acceptable. This cap is a safety valve, not a permanent limit.

### Message Franking (Open Decision 3)

Deferred to v1.5. Decide after Phase 3 telemetry on abuse-report volume. The current report mechanism is reporter-side plaintext disclosure with explicit UX consent.

### Read Receipts / Typing Indicators in E2EE (Open Decision 7)

Off by default. If shipped, they travel inside the MLS ciphertext — never as server-readable metadata.

### Automated Moderation Bots in Encrypted Channels (Open Decision 9)

Allowed as visible members (workspace opt-in, disclosed in member list and channel header). A bot is a member device that sees plaintext like any member — not a hidden recipient. If the bot host co-resides with the message server, this rebuilds escrow with visibility; custody requirements must be defined.

---

## Web-Client Exclusion (Open Decision 5)

Web clients are **permanently excluded from E2EE participation in v1**. Revisit only as an explicitly disclosed degraded trust tier.

### Rationale: Code-Delivery Attack

This is a **code-delivery property**, not a protocol or JS/DOM limitation. OpenMLS runs in WASM, and MLS ships in comparable runtimes elsewhere (Discord DAVE). But a web page re-fetches its application code from the operator on every load. A hostile operator can serve a targeted, key-exfiltrating build to a single user on a single load and revert without trace — incompatible with adversary #1.

This hole applies to **any** E2EE protocol executed in browser-delivered code. It is not specific to MLS or to JavaScript; it is fundamental to the web trust model where the server controls the code.

### Web UX Contract

- E2EE conversations render as a **fail-closed capability state**: conversation existence may render (the server necessarily knows membership for routing), content renders as "end-to-end encrypted — open in a packaged client."
- No plaintext fallback exists.
- No server-side decryption path exists to fall back to.
- A logged-in web session of the same user has no decryption capability: keys are device-bound, never account-bound.

### Revisit Paths

- Code-Verify-style build-integrity extension for web.
- Isolated Web Apps / signed web bundles if they standardize beyond Chromium.
- Both effectively reinvent packaged distribution. Browser key storage remains weaker than OS keystores.
- Any web tier would ship as **disclosed-degraded**; parity is off the table.

---

## Packaged Client Architecture

The unified SolidJS frontend is retained across web, desktop, and (later) mobile. E2EE changes where code and keys live, not the UI stack.

### Desktop (Tauri + SolidJS)

- **Bundled assets:** UI assets are bundled inside the signed package and served from the local application protocol. Remote-loading the hosted web UI into the shell is prohibited — a webview pointed at server-delivered code is a browser with extra steps and inherits the web trust model wholesale.
- **Crypto core placement:** OpenMLS and all key/state operations run in the Rust host process — not as WASM inside the webview. The webview communicates with the core over a narrow, typed IPC surface: commands and ciphertext in, plaintext and verified state out.
- **Key isolation:** Key material never enters the JS heap. This shrinks the blast radius of any webview compromise (XSS, renderer bug, content-safety bypass) from "steal keys" to "read plaintext currently on screen."

### Mobile (aligns with main-plan Phase 9)

- Same shared Rust core over FFI (Swift/Kotlin bindings).
- Platform keystores for custody.
- Same narrow-boundary discipline between UI and core.

### Device-Bound Keys

- Decryption capability follows paired, certified devices — never account credentials.
- Account login in a non-capable client (e.g., the web app) confers nothing.
- Device certificates `(user_id, device_id, device_signature_pubkey)` are signed by the user's root identity key. Peers verify the chain to the pinned root key.

### Update Integrity

- Signed update manifests with downgrade protection.
- Reproducible builds and binary transparency per Supply Chain section.
- Code signing converts the web-model attack (silent, targeted, per-load substitution) into a release-pipeline compromise that ships an auditable artifact to every user — a necessary floor, not the full answer.

---

## Mailbox Retention Model

### Server as Delivery Mailbox

- E2EE ciphertext is retained on the server only until all member devices acknowledge delivery or a TTL expires (default 30 days, configurable), then hard-deleted.
- **No long-term server-side ciphertext archive.** This directly shrinks the operator's harvest-now-decrypt-later surface and is a headline security property.
- The client's local encrypted store (SQLCipher-or-equivalent; store key in platform keystore) is canonical history.

### New-Device History Sync

- Device-to-device encrypted history sync from an existing device (or backup restore).
- QR pairing alone grants future messages only — history sync is a separate, explicit flow.
- No server-side plaintext at any point in the sync.

### Backup Policy (Open Decision 1 → Opt-In)

- **Opt-in passphrase backup** (recommended — hard no-backup kills consumer adoption; Signal itself relented).
- Passphrase-encrypted blob (Argon2id at aggressive parameters).
- Covers identity keys + history snapshot.
- Clearly documented non-recoverability if passphrase lost.
- Backup enrollment controls in the encryption settings panel.

---

## Supply Chain and Build Integrity

### Dependency Gate

- `cargo audit` — RustSec advisory database.
- `cargo deny` — license/bans/sources validation, including exact-version MPL-2.0 exceptions for the `hpke-rs` family.
- `cargo vet` (or equivalent documented review process) — configured for OpenMLS dependencies.
- Pinned and hash-locked dependencies.
- External audit status review.

### OpenMLS License Compatibility

OpenMLS is MIT licensed. Most transitive dependencies are MIT, Apache-2.0, or
BSD licensed. The `hpke-rs`, `hpke-rs-crypto`, and `hpke-rs-rust-crypto`
packages are MPL-2.0; they are approved only at reviewed exact versions and
require the source-availability notice documented in `E2EE_SUPPLY_CHAIN.md`.

### Client Build Integrity

- Signed releases with downgrade-protected update manifests.
- Reproducible builds as a goal — third parties can verify signed binaries match public source.
- Binary transparency for client releases on the roadmap.
- Code signing is necessary but not sufficient: it proves origin, not honesty. It converts per-user, per-load targeted substitution into an all-users release-pipeline compromise producing an auditable artifact. Build machines and signing-key custody remain disclosed trust dependencies.

---

## Consequences

1. **All E2EE code paths use OpenMLS.** No bespoke ratchets, no parallel crypto stacks, no hand-rolled key schedules.
2. **MLS group operations are the core abstraction.** Create, add, remove, update, external-commit recovery, exporter secret derivation — these are the operations all E2EE features build on.
3. **The server is restricted to opaque-blob relay.** All server-side validation of MLS payloads is shape-only (size bounds, field presence, epoch monotonicity). The server never parses MLS interiors.
4. **Web clients cannot participate in E2EE.** This is a permanent v1 exclusion with a documented revisit path. Web UX for E2EE conversations is fail-closed.
5. **MLS in-group non-repudiation is an accepted property.** Deniable authentication is not pursued in v1.
6. **Ciphersuite agility is built into all wire formats.** The baseline 0x0003 can be migrated to a hybrid PQ suite without protocol-level changes.
7. **One supply-chain review covers all E2EE domains.** OpenMLS is the single crypto dependency added for E2EE.
8. **Guild encrypted channels have an initial 5k-leaf cap.** Performance must be validated before the cap is raised.

---

## OpenMLS API Patterns (Reference for Implementation)

The following patterns, validated in the Phase 0 engineering spike, are the canonical integration points:

```rust
// Provider — default crypto + storage + rand
let provider = OpenMlsRustCrypto::default();

// Credential + signature keys
let credential = BasicCredential::new(identity_bytes);
let signature_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm())?;

// KeyPackage creation
let key_package_bundle = KeyPackage::builder()
    .build(ciphersuite, provider, &signer, credential_with_key)?;

// Group creation
let mut group = MlsGroup::builder()
    .padding_size(100)
    .ciphersuite(ciphersuite)
    .use_ratchet_tree_extension(true)
    .build(provider, &signer, credential_with_key)?;

// Adding members → Welcome message
let (mls_message_out, welcome, group_info) =
    group.add_members(provider, &signer, &[key_package.key_package()])?;
group.merge_pending_commit(provider)?;

// Application messages
let mls_message_out = group.create_message(provider, &signer, message_bytes)?;

// Exporter secrets (for SFrame in Phase 5)
let secret = group.export_secret(provider.crypto(), "label", &[], 32)?;
```

See `spikes/e2ee-mls-roundtrip/` for a full lifecycle demonstration.

---

## Ratification

This ADR is **Proposed** pending maintainer ratification. Upon ratification, it becomes binding for all subsequent E2EE implementation phases. Changes to the decisions recorded here require a superseding ADR.
