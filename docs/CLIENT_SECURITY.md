# Client Security Baseline (Desktop + Web)

This document defines non-negotiable hardening controls for Filament clients.
Server-provided data is always treated as untrusted input.

## Desktop (Tauri) Baseline

- CSP must remain strict (`default-src 'none'`) and must not allow `unsafe-inline` or `unsafe-eval`.
- Remote navigation is blocked except:
  - `tauri://localhost`
  - `https://app.filament.local`
- Tauri command surface is intentionally minimal:
  - `store_session`
  - `clear_session`
  - `read_session_metadata`
  - `initialize_e2ee_store`
  - `read_e2ee_store_status`
  - `read_encryption_settings`
  - `rotate_root_identity`
- The native command host exposes exactly that audited manifest through a
  capability-oriented backend. Session identity, platform credential access,
  filesystem paths, network submission, and MLS state cannot be supplied as
  command arguments.
- Session tokens and native command state use redacted `Debug` output. IPC
  failures are closed, non-sensitive codes rather than backend error strings.
- Signed updates are required.
- Crash logs must redact all access/refresh token material.

The final Tauri adapter remains blocked on the current Tauri 2.11.5 dependency
graph: it fails this repository's advisory and license gates through
unmaintained GTK3 bindings, an unsound GLib advisory, and disallowed MPL-2.0
transitives. Do not add advisory/license exceptions to bypass this blocker.

Configuration sources:
- `apps/filament-client-desktop/tauri.conf.json`
- `apps/filament-client-desktop/security-policy.json`
- `apps/filament-client-desktop/src-tauri/src/lib.rs`

## E2EE Device Pairing Boundary

- Pairing is implemented in the native Rust E2EE core. Root identity material
  must never cross into JavaScript or a broad IPC surface.
- A new device displays a single-use QR offer containing an ephemeral X25519
  receiver key and a high-entropy pairing secret. The offer expires after at
  most five minutes and is capped at 2 KiB.
- The QR payload is sensitive physical-channel data. It must not be logged,
  included in telemetry or crash reports, or relayed through the Filament
  server.
- An existing certified device signs the pairing context with its MLS Ed25519
  device key. The root secret is encrypted with the approved OpenMLS provider's
  X25519/HKDF-SHA-256/ChaCha20-Poly1305 HPKE suite, and the response is
  authenticated under the QR secret to prevent sender substitution.
- Returning transfer payloads are capped at 4 KiB, parsed with unknown-field
  rejection, and accepted only once by the in-memory receiver state. Pairing
  restores identity only; history synchronization is a separate protocol.

## E2EE Local Storage Boundary

- E2EE state is stored by the native Rust core in a device-scoped SQLCipher
  database. The bundled SQLCipher build uses a 32-byte random database key held
  by Keychain, Windows Credential Manager, or Secret Service.
- The webview may request initialization and read a fixed readiness response.
  It cannot supply a filesystem path, user/device identity, database key, or
  record key, and no IPC response contains key material.
- Database paths are derived from native validated `UserId`/`DeviceId` values
  beneath the Tauri-provided application-data directory. Relative paths,
  symlinks, hard-linked database files, and non-regular files fail closed.
- On Unix, the E2EE directory is mode `0700` and the database is mode `0600`.
  SQLCipher plaintext headers are disabled, temporary tables remain in memory,
  secure deletion is enabled, and the rollback journal stays inside the
  private directory.
- Phase 1 caps each record at 4 MiB, each device store at 4,096 records, and the
  encrypted database at 64 MiB. Phase 4 must review these caps before adding
  full local message history.
- The key-isolation audit and negative-test inventory are recorded in
  `docs/E2EE_KEY_ISOLATION_AUDIT.md`.

## Token Storage Strategy by OS

Client auth tokens are stored only in OS-provided secure stores.

- macOS: Keychain (`macos-keychain`), service `com.filament.desktop`
- Windows: Credential Manager (`windows-credential-manager`), service `FilamentDesktop`
- Linux: Secret Service (`secret-service`), service `com.filament.desktop`

Shared account key prefix: `filament-user-`.

No plaintext token persistence in logs, local files, or crash reports is permitted.

## Web Client Baseline

- CSP is locked down and checked in source:
  - `apps/filament-client-web/security/csp.json`
- Allowed URL schemes for network access are restricted to `https` and `wss`.
- Dangerous script behaviors (`eval`, `new Function`, inline scripts) are forbidden.
- Auth routes are hosted at `/login`; authenticated shell is served at `/app` with route guards.
- Session tokens are kept in bounded `sessionStorage` payloads and re-validated on read.
- API client uses bounded JSON response parsing and request timeouts to limit malicious payload impact.

## Validation Gates

These controls are enforced by tests in:
- `apps/filament-client-desktop/src-tauri/tests/hardening_config.rs`
- `apps/filament-client-desktop/src-tauri/src/lib.rs`
- `crates/filament-e2ee/src/sqlcipher_store.rs`
- `apps/filament-client-web/tests/domain-auth.test.ts`
- `apps/filament-client-web/tests/session-storage.test.ts`
- `apps/filament-client-web/tests/routes-login.test.tsx`
