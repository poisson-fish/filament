# Filament Threat Model

Core platform contract established in Phase 0; E2EE contract established by `PLAN_E2EE.md` (v2) and binding for its implementation phases.

## Trust Boundaries
- Internet clients to Filament server.
- Filament clients consuming server-provided data (malicious server model).
- Filament server to Postgres.
- Filament server to LiveKit.
- E2EE boundaries (`PLAN_E2EE.md` contract):
  - client-held key material and local encrypted stores vs. everything else, including the server
  - device-to-device pairing and history-sync channels
  - the push notification pipeline (must never carry plaintext)
  - packaged client build and update channels

## Primary Adversaries
- Unauthenticated internet attacker.
- Authenticated abusive user.
- Malicious or compromised server sending hostile payloads to clients.
- Hostile server operator (E2EE adversary #1): full database read, permanent ciphertext archive capability, directory/membership mutation, delivery withholding/reordering, and — for web — the ability to serve malicious client code.
- Compromised or stolen client device (one-time and ongoing).
- Malicious user or device inside a conversation.
- Supply-chain compromise in dependencies or client build/update channels.

## Key Abuse Cases
- Oversized request/event DoS.
- Brute force and credential stuffing against auth endpoints.
- Refresh token replay.
- Fanout amplification and slow-consumer exhaustion.
- Upload abuse (zip bombs, MIME spoofing, path traversal attempts).
- Storage exhaustion via many small uploads from a single user.
- Orphaned attachment data causing quota/accounting drift.
- Malicious markdown/link payloads targeting client execution.
- Public directory join spam and burst join-fail probes across many workspaces.
- Public workspace enumeration attempts via join endpoint status probing.
- Spoofed `x-forwarded-for` chains to bypass IP-scoped abuse controls.
- Rejoin loops after moderation actions (user-ban and IP-ban evasion attempts).

E2EE abuse cases (`PLAN_E2EE.md`):

- Harvest-now-decrypt-later archiving of E2EE ciphertext.
- Ghost user/device injection via directory or membership mutation.
- First-contact key substitution during the TOFU window.
- KeyPackage pool exhaustion and claim flooding.
- Commit storms and epoch-conflict thrashing against the Delivery Service.
- Ciphertext withholding, reordering, or selective delivery by the server.
- Crypto-mode downgrade attempts and server-flipped trust indicators.
- Push-pipeline plaintext leakage.
- Passphrase-backup brute force.
- Malicious or coerced device-pairing attempts.
- SFU-side media decryption attempts and media key leakage.
- Theft of local encrypted stores from compromised endpoints.
- Targeted per-user malicious web builds (single-load key exfiltration by the operator).
- Remote code loading in packaged clients (shell pointed at server-hosted UI).
- Release-pipeline compromise and update-channel downgrade attacks.
- Webview/renderer compromise (XSS) aimed at key material.

## Directory Join + Guild IP Moderation Threats (Phase 0 contract)
- Join spam/DoS:
  - attacker continuously submits authenticated `POST /guilds/{guild_id}/join` attempts to overload membership and audit writes.
  - mitigation contract: explicit per-IP + per-user join caps, bounded audit page size, and bounded guild IP-ban record count.
- Workspace enumeration:
  - attacker probes random guild IDs and compares private/nonexistent behavior for membership oracle extraction.
  - mitigation contract: policy-consistent responses for private or nonexistent join targets (`404 not_found`) and no visibility disclosures outside public directory list results.
- Forwarded-IP spoofing:
  - attacker sets forged `x-forwarded-for` to avoid IP moderation/rate limits.
  - mitigation contract: trusted proxy mode is opt-in; default uses socket peer address; forwarded header parsing is strict and canonicalized.
- Rejoin abuse after moderation:
  - attacker cycles accounts and IPs to immediately rejoin after bans.
  - mitigation contract: join path checks both user bans and guild IP bans, records auditable rejection reason, and stores user-IP observations for server-side matching without owner IP exposure.

## E2EE Threats (`PLAN_E2EE.md` contract)
- Hostile operator content access:
  - adversary reads Postgres and stored blobs directly for `mls_v1` conversations.
  - mitigation contract: MLS (RFC 9420) E2EE; server stores opaque `PrivateMessage` blobs plus a minimal routing envelope; shape-only server validation; no key escrow or server-readable "encrypted" middle mode exists in the product.
- Ciphertext archive / harvest-now-decrypt-later:
  - adversary retains all ciphertext hoping for future key compromise or quantum decryption.
  - mitigation contract: forward secrecy (per-message keys deleted after use makes an archive worthless), mailbox retention (hard-delete after all-device ack or TTL), size-bucket padding, and hybrid post-quantum key establishment on the roadmap.
- Ghost users and ghost devices:
  - adversary mutates directory or membership data to add silent readers or inject devices.
  - mitigation contract: membership changes are member-signed MLS proposals/commits verified at every client; device lists are root-key-certified and the server never holds root keys; the server's external-sender role is restricted to `Remove` proposals; clients hard-reject externally proposed `Add`s.
- Delivery withholding and reordering:
  - adversary drops, delays, reorders, or selectively delivers ciphertext.
  - mitigation contract: detectable, not preventable — MLS epoch + per-sender generation counters drive a "messages may be missing" indicator; stalled Remove commits trigger a bounded-latency fail-closed send block. Documented residual risk.
- Downgrade and trust-indicator spoofing:
  - adversary flips `crypto`/routing hints or serves plaintext records into an E2EE conversation to trick clients or users.
  - mitigation contract: crypto mode is a pinned conversation/channel invariant; encryption badges derive only from local cryptographic verification; server-field/local-verification mismatches fail closed and never fall back to plaintext.
- Endpoint compromise:
  - one-time key/state theft or ongoing device compromise.
  - mitigation contract: post-compromise security via MLS update commits bounds one-time theft; first-class device removal performs cryptographic eviction from all groups; `Rotate identity` provides continuity reset; platform keystores and encrypted local stores raise theft cost. Content already decrypted on a compromised endpoint is out of scope.
- Web-served malicious client code:
  - operator serves key-exfiltrating JavaScript — including a targeted build to a single user on a single page load, reverted without trace. This is a code-delivery attack, not a protocol weakness; it applies to any E2EE protocol executed in browser-delivered code.
  - mitigation contract: web clients are excluded from E2EE in v1; E2EE code executes only in signed packaged builds with locally bundled assets; packaged clients are prohibited from remote-loading application code from the server; web rendering of E2EE conversations is a fail-closed capability state with no plaintext fallback.
- Release-pipeline and update-channel compromise:
  - adversary compromises build infrastructure or signing keys to ship a malicious signed build, or downgrades clients to a vulnerable version.
  - mitigation contract: signing converts silent per-user substitution into an all-users, auditable artifact; signed update manifests with downgrade protection; reproducible builds enable third-party source-to-binary verification; binary transparency is roadmapped. Residual: build machines and signing-key custody remain disclosed trust dependencies.
- Webview compromise in packaged clients:
  - attacker achieves script execution in the client webview (renderer bug, content-safety bypass) and attempts key theft.
  - mitigation contract: MLS state and key operations are confined to the native Rust core behind a narrow, typed IPC surface (ciphertext in, plaintext out); key material never enters the JS heap; safe-token markdown rendering limits the injection surface. Residual: a compromised webview can read plaintext currently rendered on screen.
- First-contact impersonation:
  - operator substitutes key material on first contact (TOFU window).
  - mitigation contract: root-key pinning, key-change warnings (blocking for previously verified contacts), safety-number/QR verification; key transparency (Phase 8) converts silent equivocation into detectable, one-time lying.
- KeyPackage and commit resource abuse:
  - attacker floods KeyPackage claims/uploads or spams commits to exhaust pools and thrash epochs.
  - mitigation contract: bounded pool sizes, last-resort package semantics, per-user/per-device/per-route rate limits, claim audit logging, single-writer-per-epoch ordering with deterministic `409 epoch_conflict`, and commit-storm backpressure.

## E2EE Security Goals
- Server cannot decrypt content for `mls_v1` conversations.
- Forward secrecy: compromise at time T reveals nothing sent before T.
- Post-compromise security: one-time state theft stops working after the victim's next update/commit round trip.
- Membership integrity: the server can never expand a group's read audience; server-initiated changes are removal-only and client-validated.
- Encryption-state integrity: trust indicators derive from local verification only, never from server-supplied fields.
- Delivery integrity: withheld or dropped messages are detectable.
- Retention minimization: server holds E2EE ciphertext transiently (mailbox model), shrinking the harvest surface.

## Mandatory Mitigations (Phase 0 baseline)
- Global request body cap and request timeout.
- Baseline per-IP rate limiting.
- Strict protocol envelope with version checks and max message size.
- Structured logging and request IDs for incident correlation.
- CI supply-chain gates (`cargo audit`, `cargo deny`, dependency review, SBOM).
- Directory moderation contract hard limits:
  - join endpoint: bounded per-IP and per-user request rates
  - audit list endpoint: strict cursor format + max page limit
  - guild IP-ban endpoints: bounded list/apply/remove limits and strict reason/expiry validation
- E2EE contract hard limits (design-locked; enforced from the owning `PLAN_E2EE.md` phase):
  - strict size caps on KeyPackages, commits, Welcomes, proposals, and message envelopes
  - KeyPackage pool caps, claim rate limits, and claim audit logging
  - single-writer-per-epoch commit ordering with deterministic `epoch_conflict` rejection
  - mailbox TTL and all-device-ack garbage collection
  - `cargo vet` (or equivalent) added to supply-chain gates for E2EE dependencies
  - zero key material in logs, telemetry, tracing, or crash dumps

## Residual Risks (Disclosed)
- First-contact key claims are TOFU until users verify safety numbers or key transparency ships (Phase 8).
- The server can withhold or reorder ciphertext — detectable, not preventable.
- Packaged-client build integrity is a trust dependency: build machines and signing keys can be compromised. Mitigated by signed releases, downgrade-protected updates, reproducible builds, and (roadmap) binary transparency; signing alone proves origin, not honesty.
- A compromised webview/renderer can read plaintext currently displayed, even though key material is isolated in the native core.

## Out of Scope (Current)
- Federation trust relationships.
- Multi-region distributed sharding.
- E2EE non-goals (explicit, by design):
  - hiding routing metadata from the server (who talks to whom, when, approximate message frequency)
  - protecting content on a compromised endpoint after decryption
  - traffic-analysis resistance beyond size-bucket padding
  - post-quantum signatures in v1 (hybrid-PQ key establishment is roadmapped; authentication attacks cannot be harvested)

Scope change note: end-to-end encryption for DMs, group DMs, guild channels, and media — previously out of scope — is now a phased roadmap item. See `PLAN_E2EE.md`.
