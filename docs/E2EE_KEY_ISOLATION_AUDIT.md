# E2EE Key-Isolation Audit — Phase 1

**Date:** 2026-07-19
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

## Native Data Flow

1. The native authenticated session supplies typed `UserId` and `DeviceId`
   values to `LocalStoreId`; these values are not accepted from store IPC.
2. `DesktopE2eeStore` derives the database filename beneath the host-provided
   application-data directory.
3. `OsStoreKeyProvider` loads or creates a random 32-byte key in the fixed
   `com.filament.desktop.e2ee-store` service using the OS credential store.
4. `SqlCipherKeyStore` applies that key before the first database read, verifies
   that SQLCipher is active, and retains the connection behind the native
   `LocalKeyStore` interface.
5. IPC receives only `{ ready, backend, key_custody }`; none of those fields is
   secret or a storage locator.

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
- IPC policy contains no private-key display, copy, export, database read, or
  generic filesystem command.

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
- `apps/filament-client-desktop/src-tauri/tests/hardening_config.rs`: exact IPC
  allowlist alignment with `security-policy.json`.

## Remaining Release Checks

- Exercise session write/read/logout and SQLCipher-key custody against each real
  OS credential backend from signed macOS, Windows, and Linux packages;
  headless and offline-launch CI do not claim this platform smoke coverage.
- The exact Tauri 2.11.5 exceptions approved on 2026-07-22 and recorded in ADR
  0002 remain temporary. Patchable findings remain denied, and the
  GTK3/GLib/MPL exception set must be reviewed on every Tauri release and no
  later than 2027-01-18.
- Re-run the audit whenever a Tauri command, serialization type, key-storage
  backend, crash reporter, or E2EE logging path changes.
- Phase 4 must review the 64 MiB/4,096-record foundation limits and add the
  canonical encrypted history schema without exposing general-purpose IPC.
