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
- The runtime backend returns typed `unavailable` errors until production
  session, transport, mailbox, and MLS coordination are injected. It never
  claims secure storage or messaging is ready.
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

Release bundles omit `--debug` and require the platform signing gates. No
signing credentials belong in the repository.

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

This is host-scaffold evidence, not the Phase 5.5 messaging exit suite.
Production E2EE messaging and mobile platform custody remain fail-closed work.
