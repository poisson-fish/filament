# E2EE Key-Isolation Audit — Phase 1

**Date:** 2026-07-23
**Scope:** desktop SQLCipher key custody and the Rust/webview boundary

## Result

The Phase 1 key-isolation boundary passes its source and negative-test audit.
MLS/root/database key material remains in native Rust. The desktop webview can
request encrypted-store initialization, read fixed readiness and public
settings responses, and confirm an identity rotation; it has no command that
returns or accepts key material, database paths, record keys, or raw E2EE
state.

The native command host is covered by tests that exercise boundary validation,
redaction, and exact alignment with the audited command manifest. The native
backend remains capability-oriented: the webview cannot provide a filesystem
path, credential account, native user/device identity, MLS state, or key
material to any command.

The packaged runtime now persists the validated access/refresh pair as one
versioned credential record under fixed native service/account identifiers.
The record is capped at 12 KiB, revalidated on load, rejects unknown fields and
versions, and uses zeroizing native buffers. Session metadata exposes only
presence and access-token expiry; tokens cannot be read back through IPC.
Authenticated identity discovery uses one compile-time HTTPS authority with
redirects disabled and 256 KiB request/response caps. A separate bounded
native credential record maps authenticated account IDs to host-created device
IDs; neither value can be selected by the webview.

## Native Data Flow

1. The stored access token authenticates `/auth/me` through the pinned native
   origin. The validated server `UserId` and native registry `DeviceId` supply
   `LocalStoreId`; neither value is accepted from store IPC.
2. `DesktopE2eeStore` derives the database filename beneath the host-provided
   application-data directory.
3. `OsStoreKeyProvider` loads or creates a random 32-byte key in the fixed
   `com.filament.desktop.e2ee-store` service using the OS credential store.
4. `SqlCipherKeyStore` applies that key before the first database read, verifies
   that SQLCipher is active, and retains the connection behind the native
   `LocalKeyStore` interface.
5. IPC receives only `{ ready, backend, key_custody }`; none of those fields is
   secret or a storage locator.
6. Fresh-account enrollment atomically compare-and-inserts the root secret,
   complete OpenMLS provider checkpoint, and exact pending KeyPackage upload
   into SQLCipher. The outbox survives uncertain network results and is
   removed only after a confirmed idempotent upload.
7. Destructive identity rotation validates the authenticated public continuity
   chain against the local pin and sequence. A fresh replacement root, device
   signer/provider checkpoint, and KeyPackage outbox are written as one
   encrypted pending record before submission. Exact retries are idempotent;
   local adoption atomically replaces the root, resets MLS groups for
   authenticated recovery, and retains the upload outbox before clearing the
   retry record.
8. The packaged host derives mailbox routes and root pins from that checkpoint,
   reads fixed same-origin commit/message endpoints, and invokes the durable
   native coordinator. MLS state, authenticated history, and an acknowledgment
   outbox commit atomically; a lost response leaves the outbox for exact retry
   before any later page. No group, device, root pin, or plaintext enters IPC.
9. The bundled SolidJS client invokes only the generated seven-command ACL.
   Session adoption follows successful native custody, settings decode only
   exact public fields, and root rotation accepts only the fixed typed
   confirmation. Native failures map to closed codes without reflecting host
   or server text.

## Guardrails Verified

- Crate-level `#![forbid(unsafe_code)]` remains enabled.
- Root-secret byte access remains crate-private in `filament-e2ee`.
- The database key is returned only by the native `StoreKeyProvider` trait in a
  zeroizing buffer and is never serializable.
- SQLCipher plaintext headers are disabled. Tests verify that neither the
  SQLite magic header nor a stored 32-byte fixture appears in the database.
- A wrong database key fails closed; no empty replacement database is created.
- Relative paths, symlinks, hard-linked files, non-regular files, oversized
  databases, oversized values, and invalid record identifiers are rejected.
- Debug and error output omits credential accounts, paths, values, and keys.
- Session writes cannot select a credential service/account or create a
  mismatched access/refresh pair; logout deletion is idempotent.
- The native REST origin cannot be supplied over IPC, redirects are disabled,
  bearer headers are marked sensitive, and hostile/oversized response fields
  cannot select an account, device, path, or credential identifier.
- Automatic enrollment is limited to an account whose authenticated public
  device directory is empty. Accounts with existing devices require the
  separately authenticated pairing flow; root replacement fails closed.
- Every returned device certificate is checked against the locally persisted
  root before settings or readiness is exposed. Every returned rotation chain
  is checked for exact length, continuity, dual signatures, monotonic sequence,
  and the local pin.
- Replacement root and device secrets remain inside zeroizing native values
  and SQLCipher records. Lost rotation responses leave a durable candidate;
  they cannot cause generation of a second replacement identity.
- Mailbox URLs use only native checkpoint group/device identifiers and the
  compile-time authority. Reads, pages, groups per pass, response bodies, and
  acknowledgment batches are bounded; hostile routing hints are authenticated
  by MLS before history is stored or acknowledged.
- IPC policy contains no private-key display, copy, export, database read, or
  generic filesystem command.
- Webview tests reject extra native response fields, duplicate/current-device
  inconsistencies, malformed public identifiers, and inexact destructive
  confirmations before values reach presentation state.

## Automated Evidence

- `crates/filament-e2ee/src/sqlcipher_store.rs`: encrypted round-trip/reopen,
  wrong-key rejection, plaintext scan, path attacks, caps, and debug redaction.
- `crates/filament-e2ee/src/keystore.rs`: typed key invariants, zeroizing loads,
  root identity persistence, and value caps.
- `apps/filament-client-desktop/src-tauri/src/lib.rs`: native-only store setup,
  status serialization negative test, fixed OS-keyring service, and redacted
  debug output.
- `apps/filament-client-desktop/src-tauri/src/tauri_host.rs`: native command
  backend boundary, validation, opaque errors, redaction, and exact-manifest
  negative tests.
- `apps/filament-client-desktop/src-tauri/src/session_store.rs`: bounded
  single-record session custody, strict reload validation, zeroization, and
  redacted credential-store diagnostics.
- `apps/filament-client-desktop/src-tauri/src/native_api.rs`: pinned HTTPS
  origin policy, bounded strict DTO handling, sensitive authorization headers,
  ghost-device/root-replacement rejection, and fixed mailbox/ack transports.
- `apps/filament-client-desktop/src-tauri/src/native_gateway.rs`: pinned WSS
  derivation, bearer-header authentication, strict bounded wake decoding,
  coalesced group queue, and redacted connector diagnostics.
- `apps/filament-client-desktop/src-tauri/src/runtime.rs`: bounded native
  realtime wake/periodic reconciliation, commit-before-message coordination,
  and lost-response acknowledgment retry.
- `apps/filament-client-web/src/lib/native-client.ts`: exact native command
  requests, strict public-response decoding, and fixed redacted error mapping.
- `apps/filament-client-web/src/features/app-shell/controllers/native-encryption-controller.ts`:
  fail-closed custody/store initialization and settings/rotation state.
- `apps/filament-client-desktop/src-tauri/src/device_registry.rs`: fixed,
  bounded native account/device bindings with duplicate and corruption
  rejection.
- `crates/filament-e2ee/src/persistence.rs`: atomic initial root/provider/
  KeyPackage-outbox persistence and restart-safe retry coverage.
- `apps/filament-client-desktop/src-tauri/tests/hardening_config.rs`: exact IPC
  allowlist alignment with `security-policy.json`.

## Remaining Release Checks

- Exercise session write/read/logout and SQLCipher-key custody against each real
  OS credential backend from signed macOS, Windows, and Linux packages;
  headless and offline-launch CI do not claim this platform smoke coverage.
- Exercise authenticated first-device publication and KeyPackage upload
  against packaged clients and the PostgreSQL server; unit tests use a hostile
  in-process transport fixture and do not claim packaged-network evidence.
- The exact Tauri 2.11.5 exceptions approved on 2026-07-22 and recorded in ADR
  0002 remain temporary. Patchable findings remain denied, and the
  GTK3/GLib/MPL exception set must be reviewed on every Tauri release and no
  later than 2027-01-18.
- Re-run the audit whenever a Tauri command, serialization type, key-storage
  backend, crash reporter, or E2EE logging path changes.
- Phase 4 must review the 64 MiB/4,096-record foundation limits and add the
  canonical encrypted history schema without exposing general-purpose IPC.
