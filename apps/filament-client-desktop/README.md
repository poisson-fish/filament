# Filament Packaged Clients

This directory packages the locally built SolidJS client with the shared
Tauri 2 host. The native process owns the security boundary. The webview has
no filesystem, shell, updater, opener, clipboard, or general network plugin.

## Current increment

- Tauri `2.11.5` and Tauri CLI `2.11.4` are pinned in lockfiles.
- The `main` webview can invoke only the seven commands in
  `security-policy.json`; generated application ACL permissions enforce the
  same list.
- Native IPC request bodies are capped at 16 KiB before command dispatch.
- Runtime navigation permits only Tauri's exact local-bundle origins. The
  HTTPS development origin is compiled in only for development builds.
- CI builds the exact Ubuntu 24.04 x86-64, macOS 15 Apple-silicon and Intel,
  Windows x86-64, Android API 36 arm64, and iOS 17 Apple-silicon simulator
  paths declared in `platform-support.json`.
- A cross-platform artifact gate rejects missing or duplicate formats,
  symlinks, secret-like/source-map assets, oversized bundles, remote HTML
  scripts, missing macOS notices, and emits deterministic SHA-256 evidence.
- Desktop CI installs the Debian and MSI packages, mounts the macOS disk image,
  and launches those paths plus the AppImage with dead proxy endpoints. A
  bounded process-tree probe rejects early exit, output floods, or any network
  socket before emitting redacted launch evidence.
- The runtime stores one versioned, bounded access/refresh session record in
  the platform credential service under fixed native identifiers. Logout
  deletes that record idempotently; malformed stored records fail closed and
  token buffers are zeroized on drop.
- The locally bundled SolidJS client calls only the audited native command
  manifest through the exact, zero-transitive-dependency Tauri JavaScript API
  package.
  Login and refresh must establish native custody before the UI adopts a
  session; logout clears native custody even when remote teardown is
  unavailable. Native responses are decoded with exact fields and bounds.
- The native host discovers the authenticated user through the compile-time
  `FILAMENT_NATIVE_API_ORIGIN` HTTPS authority (default
  `https://api.filament.local`). Redirects are disabled, bearer headers are
  sensitive/redacted, requests and ordinary responses are capped at 256 KiB,
  and strict server DTO validation fails closed. Mailbox responses have a
  separate 2 MiB encoded-body cap around the protocol's 256 KiB aggregate
  opaque-blob limit.
- A server account with an empty certified-device directory can enroll its
  first device. The host creates the device ID itself, atomically persists the
  root identity, complete MLS provider, and retryable KeyPackage upload outbox
  in SQLCipher, publishes the certificate, and clears the outbox only after a
  confirmed idempotent upload. An account that already has devices remains
  pairing-gated; the runtime never creates a replacement root.
- Encrypted-store readiness and public encryption settings are now backed by
  the authenticated native device. The destructive root-identity command now
  verifies the bounded public continuity chain, persists a replacement root,
  signer/provider checkpoint, and KeyPackage outbox before submission, and
  reconciles an exact idempotent retry after a lost response or restart.
  Successful rotation resets MLS groups for authenticated external-commit
  recovery and never exposes replacement secrets to the webview. The bundled
  settings panel now initializes this backend, renders only its redacted
  fingerprint/device snapshot, and enforces the exact destructive
  confirmation before invoking rotation.
- On initialization and authenticated settings refresh, the host drains a
  bounded rotating set of locally authenticated DM/group-DM proposal, commit,
  message, and attachment mailboxes. It submits any prior durable
  acknowledgment before another read, commits MLS state/history/outboxes
  atomically in SQLCipher, and clears an outbox only after an idempotent server
  response. Rejected MLS data stops that group without exposing plaintext or
  adding IPC.
- After native initialization, a native WSS listener authenticates with a
  sensitive bearer header against the compile-time server authority.
  User-bound `ready` is required before strict `mls_message`, `mls_commit`,
  `mls_welcome`, or `mls_proposal` events can enqueue an immediate group
  mailbox drain. Wakeups are coalesced in a 128-group queue; event fields
  remain untrusted routing hints. The transport rejects binary and oversized
  frames, missing or repeated readiness, idle connections, and distinct wake
  overflow.
- A native-only 15-second scheduler remains as offline/missed-event
  reconciliation. It skips contended passes and exponentially backs failed
  passes off to five minutes. Gateway reconnects begin after one second and
  cap at 30 seconds; token expiry, session replacement, and logout interrupt
  the old connection. No listener input can select an identity, destination,
  path, or key.
- Native attachment preparation now encrypts and chunks one exact-bucket
  upload per group into SQLCipher before submission. Response loss retries the
  identical opaque bytes. After upload acceptance, the host authenticates the
  private descriptor inside a retry-safe MLS attachment event and removes the
  upload record only after exact local authenticated history proves message
  acceptance.
- Conversation provisioning, encrypted conversation send/presentation UI,
  encrypted attachment composition/presentation UI, pairing UI, packaged
  end-to-end smoke coverage, and mobile platform custody evidence remain fail
  closed.
- The PostgreSQL-backed server integration suite now exercises a real
  loopback gateway with bearer-header authentication, an immediate
  established-DM wake and OpenMLS mailbox decryption, then disconnect/offline
  durable-mailbox recovery. It also proves no matching plaintext-table write
  and no fanout to a certified device outside the MLS leaf map. This does not
  yet launch or certify an installed desktop package.
- Calls and automatic updates remain disabled.

## Developer commands

Install both locked JavaScript dependency sets:

```bash
npm --prefix ../filament-client-web ci
npm ci
```

Build a local desktop package from this directory:

```bash
npm run build -- --debug --bundles app  # macOS app
mkdir -p ../../target/debug/bundle/dmg
hdiutil create -volname Filament \
  -srcfolder ../../target/debug/bundle/macos/Filament.app \
  -format UDZO ../../target/debug/bundle/dmg/Filament_local.dmg
npm run build -- --debug --bundles deb,appimage  # Linux
npm run build -- --debug --bundles msi  # Windows
```

Self-hosted builds may pin a different native API authority at compile time,
for example `FILAMENT_NATIVE_API_ORIGIN=https://chat.example npm run build`.
Only one HTTPS authority with no path, credentials, query, or fragment is
accepted. The server origin is never selectable through IPC.

Release bundles omit `--debug` and require the platform signing gates. No
signing credentials belong in the repository.

Run the bounded offline launch probe against an already-built desktop
executable:

```bash
npm run smoke:desktop -- \
  --platform macos \
  --executable ../../target/debug/bundle/macos/Filament.app/Contents/MacOS/filament \
  --evidence ../../target/package-evidence/local-macos-launch.json
```

Use `linux` or `windows` only on the matching host. The probe accepts only a
regular executable, passes a minimal environment with dead proxy endpoints,
samples the complete process group for network sockets, caps captured output,
and terminates the process after an eight-second observation.

Mobile project generation uses the same Rust entry point:

```bash
npm run android:init -- --ci
npm run ios:init -- --ci
```

Android CI pins API 36, build-tools 36.0.0, NDK 27.2.12479018, Java 21, and
the `aarch64-linux-android` Rust target. It regenerates the Tauri Android
project from the locked CLI, verifies the API 33 minimum/API 36 target and
cleartext-traffic denial, then builds and checksums `.apk` and `.aab` paths.
Release signing credentials remain external to the repository.

The iOS CI gate uses full Xcode on macOS 15, pins the
`aarch64-apple-ios-sim` Rust target, regenerates the Xcode project from the
locked CLI, verifies the iOS 17 deployment floor and exact application
identifier, and builds an unsigned Apple-silicon simulator `.app`. The shared
artifact verifier requires both `.app` and `.ipa` for `aarch64` device-release
evidence, so simulator success cannot be mistaken for a signed device package.

Device and App Store builds require an Apple Developer team and credentials
outside the repository. Supply `APPLE_DEVELOPMENT_TEAM` plus either the App
Store Connect API variables (`APPLE_API_ISSUER`, `APPLE_API_KEY`,
`APPLE_API_KEY_PATH`) or the manual-signing variables (`IOS_CERTIFICATE`,
`IOS_CERTIFICATE_PASSWORD`, `IOS_MOBILE_PROVISION`) from a protected release
environment, then run:

```bash
npm run ios:build -- --target aarch64 --ci --export-method app-store-connect
```

The resulting signed `.app` and `.ipa` must pass `verify:package` with
`--platform ios --architecture aarch64` before distribution. Signing
credentials and provisioning profiles must never be copied into the source
tree or local web bundle.

The current development host has Android SDK 35 but lacks the pinned NDK, API
36, build-tools 36, and Android Rust target. It also has only Apple
command-line tools, not full Xcode or an iPhone SDK. Those local toolchain
gaps do not change the required Android target or the now-CI-exercised,
feasibility-gated iOS simulator path. Signed device/IPA evidence remains
pending external Apple credentials and owner review.

## Verified evidence

On 2026-07-22, an unsigned local macOS `Filament.app` built successfully,
launched with dead HTTP/HTTPS proxy endpoints, stayed alive, opened no network
socket during the launch probe, and emitted no runtime error. The artifact
contained the executable, application metadata, icon, embedded local assets,
and `THIRD_PARTY_NOTICES.txt`.

This is host/session/bootstrap-scaffold evidence, not the Phase 5.5 messaging
exit suite.
The same probe is now required for Debian, AppImage, macOS disk-image, and MSI
CI artifacts. The real-server/native-core messaging regression is not
packaged-artifact evidence until the installed application drives that same
flow through its production host boundary. Production E2EE messaging, upgrade
semantics, and mobile platform custody smoke coverage remains fail-closed work;
Android and iOS simulator launch tests are intentionally deferred.
