# ADR 0002: Packaged Client Targets and Host Runtime

- **Status:** Accepted for Phase 5.5 implementation
- **Date:** 2026-07-22
- **Deciders:** Filament maintainers
- **Supersedes:** The mobile FFI direction in ADR 0001
- **Binding for:** E2EE Phase 5.5 packaged clients
- **Machine-readable contract:** [`platform-support.json`](../../apps/filament-client-desktop/platform-support.json)

## Context

Phase 5.5 must ship locally bundled E2EE clients for Linux, macOS, Windows,
and Android. iOS remains an explicit feasibility-gated target. The same Rust
MLS core must own device identity, encrypted storage, mailbox processing, and
media keys on every target. UI code is untrusted and cannot choose native
identity, storage paths, credential-store entries, keys, or arbitrary network
destinations.

The repository already has a seven-command capability-oriented desktop host,
but it intentionally has no Tauri runtime dependency. Tauri 2.11.5 remains the
current crates.io release as of this decision. Its reviewed graph fails the
repository policy because it contains unmaintained packages, active RustSec
findings, and MPL-2.0 packages outside the three exact HPKE exceptions. The
runtime cannot be added by weakening `cargo-deny.toml` or ignoring advisories.

## Decision

Use one locally bundled SolidJS application and Tauri v2 host architecture for
desktop and mobile. Tauri mobile is selected over a separate Swift/Kotlin FFI
layer so Filament keeps one typed command boundary and does not introduce a
project-owned foreign-memory ABI. Runtime scaffolding begins only after a
Tauri release and resolved lockfile pass `cargo audit`, `cargo deny`, and
`cargo vet`; no dependency exception is pre-approved by this ADR.

The existing seven-command manifest is not implicitly expanded. Production
messaging needs additional capability operations, but their command/event
shape is a separate owner-reviewed decision under the privileged-API stop
trigger.

### Initial target contract

| Target | Minimum | Architectures | Developer/release artifacts |
|---|---|---|---|
| Linux | Ubuntu 24.04 LTS | x86-64 | `.deb`, AppImage |
| macOS | macOS 15 | Apple silicon, x86-64 | architecture-scoped `.app`, `.dmg` |
| Windows | vendor-supported Windows 11 | x86-64 | `.msi` |
| Android | Android 13 / API 33; target API 36 | arm64-v8a | signed `.apk`, `.aab` |
| iOS | iOS 17 | arm64 device and Apple-silicon simulator | signed `.app`, `.ipa` when feasibility gates pass |

Architecture-scoped macOS artifacts are selected instead of a universal
binary because native SQLCipher and LiveKit/libwebrtc inputs must each be
verified for the exact architecture. This avoids merging independently built
native artifacts before their linkage and signing evidence exists.

Ubuntu 24.04 is the Linux baseline because it is the already exercised
WebKitGTK/native-media environment and remains an LTS release. The `.deb` is
the installable baseline; AppImage is the portable secondary artifact and must
be built on the oldest supported base system. Windows uses native Windows CI
for MSI generation rather than relying on less-tested cross-compilation.

Android targets API 36 to meet the Google Play requirement beginning August
31, 2026. The lower API 33 floor limits the first release to a substantially
smaller, modern security/lifecycle matrix. iOS 17 is a product floor rather
than the oldest deployment target supported by Xcode.

Each signed client release receives security fixes for 12 months. Every OS
baseline is reviewed at least every 180 days, with at least 180 days' notice
before a planned minimum-version increase. A target must remain under vendor
security support; an earlier vendor support cutoff overrides the notice period
and this policy never extends support for an unpatched OS just to preserve
compatibility.

### Shipping controls

- Application assets originate only from the signed local bundle. Remote
  scripts, navigation, application bundles, and dynamic code updates remain
  prohibited.
- Automatic updates remain disabled until Phase 7 adds signed manifests and
  downgrade protection.
- Media is disabled on every target until that exact packaged artifact passes
  the Phase 5 media, permission, lifecycle, and teardown probes.
- Key custody is Keychain on Apple platforms, Credential Manager on Windows,
  fail-closed Secret Service on Linux, and Android Keystore on Android.
- A missing keystore, unsupported runtime, or failed encrypted-store open is a
  typed unavailable state. There is no plaintext storage or messaging fallback.

## Rejected alternatives

### Add Tauri 2.11.5 with policy exceptions

Rejected. This would weaken explicit advisory and license gates in order to
package the client, contrary to the Phase 5.5 plan and project directives.

### Separate Swift/Kotlin FFI shells

Not selected for the initial implementation. A native FFI bridge introduces a
second public boundary, generated bindings, foreign-memory ownership, and
platform-specific lifecycle glue. The bridge would also require an explicit
review of generated or handwritten unsafe Rust. It remains a contingency only
after a maintainer approves that stop-trigger decision and the approach proves
safer or more maintainable than the Tauri mobile graph.

### Broader initial OS and architecture coverage

Deferred. Linux arm64, Windows arm64, Android 32-bit ABIs, older Apple OS
versions, and universal macOS binaries multiply native dependency, keystore,
installer, and hostile-server test combinations. They may be added only with
the same package-level security evidence as the initial matrix.

## Consequences

The target and artifact matrix is now stable enough to build CI and packaging
contracts, but installable Tauri apps remain supply-chain blocked. Work that
does not require the runtime can continue: local-bundle integrity checks,
typed backend design, target-specific configuration, signing placeholders,
artifact inspection, and packaged smoke-test harnesses. The next privileged
command expansion must be reviewed before implementation.

## Authoritative baseline references

- [Tauri prerequisites and platform toolchains](https://v2.tauri.app/start/prerequisites/)
- [Tauri distribution formats](https://v2.tauri.app/distribute/)
- [Tauri AppImage baseline constraints](https://v2.tauri.app/distribute/appimage/)
- [Tauri Windows installer constraints](https://v2.tauri.app/distribute/windows-installer/)
- [Apple Xcode SDK and deployment-target matrix](https://developer.apple.com/xcode/system-requirements)
- [Google Play target API requirements](https://developer.android.com/google/play/requirements/target-sdk)
