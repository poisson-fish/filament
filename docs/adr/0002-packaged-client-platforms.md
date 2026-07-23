# ADR 0002: Packaged Client Targets and Host Runtime

- **Status:** Accepted with exact Tauri 2.11.5 exceptions
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
latest crates.io and signed upstream release as of this decision. Its reviewed
graph contains unmaintained packages, one GLib unsoundness advisory, MPL-2.0
packages outside the three HPKE exceptions, and one LLVM-exception license.
On 2026-07-22 the maintainer explicitly accepted those exact temporary risks
so packaged-client work can proceed.

## Decision

Use one locally bundled SolidJS application and Tauri v2 host architecture for
desktop and mobile. Tauri mobile is selected over a separate Swift/Kotlin FFI
layer so Filament keeps one typed command boundary and does not introduce a
project-owned foreign-memory ABI. Tauri is pinned to 2.11.5 and its resolved
lockfile must pass `cargo audit`, `cargo deny`, and `cargo vet` under the exact
exceptions below. Patchable findings are not accepted.

### Exact temporary exception scope

The license exceptions cover only `cssparser 0.36.0`,
`cssparser-macros 0.6.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0`,
`selectors 0.36.1`, and `target-lexicon 0.12.16`. MPL source-availability
notices are required for distributed packages.

RustSec exceptions cover the unmaintained GTK3 binding family
(`RUSTSEC-2024-0411` through `RUSTSEC-2024-0420`), GLib 0.18.5 iterator
unsoundness (`RUSTSEC-2024-0429`), unmaintained `proc-macro-error` and `paste`,
and the five unmaintained `rust-unic` packages selected by `urlpattern`.
Filament does not call the affected `glib::VariantStrIter` API. This narrows
exposure but does not repair the transitive unsound implementation.

The exceptions do not cover the patchable `anyhow`, `time`, or `quick-xml`
advisories present in Tauri's published-package lock. Filament must resolve
fixed compatible releases and continues to fail CI if those IDs appear.
Exceptions expire for review on 2027-01-18 and must be reconsidered for every
Tauri upgrade, whichever occurs first.

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

### Broad or version-range Tauri exceptions

Rejected. The maintainer approval applies only to enumerated crate versions
and advisory IDs. It does not generally allow MPL, LLVM exceptions,
unmaintained dependencies, unsoundness, or future Tauri graphs.

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

The target and artifact matrix is stable enough to build CI and packaging
contracts, and the exact Tauri 2.11.5 graph now backs a compiling adapter. The
adapter registers only the seven pre-existing commands, enforces a 16 KiB IPC
request cap and exact local navigation, and stores the validated access/refresh
session as one versioned, bounded platform-credential record under fixed native
identifiers. Logout deletion is idempotent; corrupt records are rejected and
token buffers are zeroized on drop. Encrypted-store initialization and all
network/MLS coordination remain typed-unavailable until authenticated device
enrollment is wired. A local macOS `.app` package launched successfully from
embedded assets on 2026-07-22 with dead network proxies.
Every resulting artifact still requires local-bundle integrity checks,
target-specific signing, and advisory/license/vet checks. CI now installs the
Debian and MSI artifacts, mounts the macOS disk image, and launches those
executables plus the AppImage with a minimal environment and dead proxy
endpoints. The bounded verifier fails on early exit, output flooding, or any
observed process-tree network socket and emits only redacted launch evidence.
This verifies desktop install/offline-launch behavior, not production E2EE
messaging or upgrade semantics. CI also regenerates the Android project from
the locked Tauri CLI, pins API 36, build-tools 36.0.0, NDK 27.2.12479018, Java
21, and the arm64 Rust target, verifies the API 33 floor/API 36 target and
cleartext denial, and produces integrity-checked local `.apk` and `.aab` paths.
Release credentials remain external. CI also regenerates the iOS Xcode project
under full Xcode,
locks the iOS 17 deployment floor, builds the Apple-silicon simulator target,
and emits integrity evidence for its unsigned `.app`. Device-release evidence
requires both the signed `.app` and `.ipa`; the verifier does not let simulator
evidence satisfy that requirement. The development host has only
`/Library/Developer/CommandLineTools`, and `xcrun --sdk iphoneos` cannot locate
an SDK, so local generation is toolchain-blocked rather than evidence that iOS
is infeasible. Signed device packaging remains gated on externally supplied
Apple Developer credentials and owner review. The next privileged command
expansion must be reviewed before implementation. Android and iOS simulator
launch suites remain deferred until mobile runtime testing is needed; their
existing build and artifact-integrity gates remain active.

## Authoritative baseline references

- [Tauri prerequisites and platform toolchains](https://v2.tauri.app/start/prerequisites/)
- [Tauri distribution formats](https://v2.tauri.app/distribute/)
- [Tauri AppImage baseline constraints](https://v2.tauri.app/distribute/appimage/)
- [Tauri Windows installer constraints](https://v2.tauri.app/distribute/windows-installer/)
- [Apple Xcode SDK and deployment-target matrix](https://developer.apple.com/xcode/system-requirements)
- [Google Play target API requirements](https://developer.android.com/google/play/requirements/target-sdk)
