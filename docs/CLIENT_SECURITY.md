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
- Signed updates are required.
- Crash logs must redact all access/refresh token material.

Configuration sources:
- `apps/filament-client-desktop/tauri.conf.json`
- `apps/filament-client-desktop/security-policy.json`
- `apps/filament-client-desktop/src-tauri/src/lib.rs`

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
- `apps/filament-client-web/tests/domain-auth.test.ts`
- `apps/filament-client-web/tests/session-storage.test.ts`
- `apps/filament-client-web/tests/routes-login.test.tsx`
