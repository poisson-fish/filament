# E2EE Supply Chain Documentation

**Phase 0 Deliverable:** Supply-chain gate configuration for OpenMLS dependencies.

## Overview

This document records the supply-chain review state for the E2EE dependency
tree (OpenMLS and its transitive dependencies). `cargo audit`,
`cargo deny --config cargo-deny.toml check`, and `cargo vet --locked` are
enforced in CI.

## OpenMLS Dependency Tree

The primary new cryptographic dependency is **OpenMLS** (MIT licensed), an RFC 9420 MLS implementation in pure Rust.

### Direct Dependencies (added in Phase 1)

| Crate | Version | License | Purpose |
|---|---|---|---|
| `openmls` | 0.8.1 + pinned upstream fix | MIT | Main MLS library — group state, KeyPackages, commits, messages |
| `openmls_rust_crypto` | 0.5.1 + pinned upstream fix | MIT | Default crypto provider (RustCrypto crates, no C deps) |
| `openmls_basic_credential` | 0.5.0 + pinned upstream fix | MIT | BasicCredential + SignatureKeyPair for credential types |
| `openmls_traits` | 0.5.0 + pinned upstream fix | MIT | Direct access to the approved provider traits used by device pairing |
| `rusqlite` | 0.39.0 | MIT | Rust SQLCipher bindings for the device-local encrypted store |
| `keyring` | 4.1.5 | MIT/Apache-2.0 | Cross-platform Keychain, Credential Manager, and Secret Service access |
| `libwebrtc` | 0.3.42 | Apache-2.0 | Optional LiveKit native WebRTC/frame-cryptor bridge for Phase 5 media E2EE |

### Key Transitive Dependencies

| Crate | License | Purpose | Policy status |
|---|---|---|---|
| `openmls_memory_storage` | MIT | Storage used by `openmls_rust_crypto` | Allowed license |
| `tls_codec` | MIT/Apache-2.0 | TLS serialization for MLS wire format | Allowed license |
| `zeroize` | Apache-2.0/MIT | Memory zeroization for key material | Allowed license |
| `serde` | MIT/Apache-2.0 | Serialization framework | Allowed license |
| `serde_bytes` | MIT/Apache-2.0 | Byte-array serialization used transitively | Allowed license |
| `rayon` | MIT/Apache-2.0 | Parallelism for tree operations | Allowed license |
| `thiserror` | MIT/Apache-2.0 | Error derive macro | Allowed license |
| `log` | MIT/Apache-2.0 | Logging facade | Allowed license |
| `getrandom` | MIT/Apache-2.0 | CSPRNG interface | Allowed license |
| RustCrypto crates (`chacha20poly1305`, `x25519-dalek`, `ed25519-dalek`, `sha2`, `hkdf`) | MIT/Apache-2.0 | Underlying cryptographic primitives | Allowed licenses |
| `hpke-rs` family | MPL-2.0 | HPKE implementation used by `openmls_rust_crypto` | Approved exact-version exception with distribution controls |
| SQLCipher / `libsqlite3-sys` | BSD-style / MIT | Bundled encrypted SQLite implementation and bindings | Allowed licenses |
| OpenSSL 3 / `openssl-src` / `openssl-sys` | Apache-2.0 and MIT/Apache-2.0 wrapper crates | Vendored SQLCipher crypto provider for consistent desktop builds | Allowed licenses |
| `keyring-core` and native store backends | MIT/Apache-2.0 | Platform credential-store adapters | Allowed licenses |
| `webrtc-sys` / `webrtc-sys-build` | Apache-2.0 | LiveKit's generated native libwebrtc bindings and pinned artifact builder | Allowed licenses; pending formal source audit |

The general `cargo-deny.toml` allowlist remains limited to MIT, Apache-2.0,
BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, and Zlib. MPL-2.0 is allowed only
for the three exact `hpke-rs` 0.7.0 packages recorded in the exceptions table.

## Current Reviewed Findings

The generated store and fresh RustSec scans identified upstream security and
maintenance findings in the OpenMLS provider chain. The vulnerability path is
resolved as follows:

- `openmls_rust_crypto 0.5.1` depends on the MPL-2.0 `hpke-rs` family. The
  maintainer approved exact-version exceptions with the binary-distribution
  controls documented below; MPL-2.0 is not generally allowlisted.
- Published `openmls 0.8.1` pins `hpke-rs 0.6.1` and vulnerable libcrux
  components. Until the next OpenMLS release, the complete OpenMLS crate family
  is patched to the exact, signed upstream dependency-fix revision
  `0e99bc8814d136f0bc7bc9ce86dd288eb32273ed`. This selects `hpke-rs 0.7.0`,
  `libcrux-sha3 0.0.10`, and `libcrux-secrets 0.0.6`; a fresh `cargo audit`
  reports no vulnerabilities.
- `proc-macro-error2 2.0.1` remains in the libcrux/hax build-time dependency
  path and is unmaintained, but has no reported vulnerability and no available
  replacement in this provider release. `cargo-deny` carries an exact
  `RUSTSEC-2026-0173` exception; the warning remains visible in `cargo audit`.
- The previous LiveKit access-token stack enabled the unused `rsa 0.9.10`
  implementation. Upgrading to `livekit-api 0.5.6` selected LiveKit's HMAC-only
  JWT provider and removed that dependency and advisory without an ignore.
- Phase 5's optional `livekit-media` feature pins `libwebrtc 0.3.42`, the
  Apache-2.0 native component published by the existing LiveKit SDK vendor.
  Filament uses its AES-GCM frame-cryptor key provider instead of defining a
  media cipher. The new package set passes the advisory and license/source
  gates. Cargo-vet exemptions record dependency intake, not a completed source
  audit; formal media-stack review remains a Phase 7 gate.

The Git patch is temporary. It must be removed in favor of crates.io packages
when OpenMLS publishes a release containing the same dependency fix.

During this review, patchable advisories were removed with bounded
lockfile updates (`crossbeam-epoch`, `lz4_flex`, `memmap2`, `quinn-proto`,
`rand`, and `rustls-webpki`). The server now uses the existing ring-backed
rustls provider and OS-native trust roots, removing vulnerable AWS-LC packages
and root-certificate data packages that did not satisfy the license policy.

## cargo-deny Configuration

The existing `cargo-deny.toml` general allowlist covers OpenMLS itself (MIT).
Three exact-version exceptions allow MPL-2.0 only for `hpke-rs 0.7.0`,
`hpke-rs-crypto 0.7.0`, and `hpke-rs-rust-crypto 0.7.0`. Version changes fail
closed until the exception and distribution notice are reviewed together.

The only OpenMLS-chain advisory exception is `RUSTSEC-2026-0173`, scoped to an
unmaintained build-time macro dependency with no published vulnerability or
available replacement. No vulnerability advisory is ignored. The obsolete
`RUSTSEC-2024-0384` exception was removed because no current package
matches it.

The source policy permits only the canonical OpenMLS Git repository. Every
OpenMLS patch entry includes the same full commit revision, and `Cargo.lock`
records its resolved commit. Unknown Git sources remain denied.

## cargo-vet Status

The generated cargo-vet store lives in `supply-chain/` and the security workflow
enforces it with `cargo vet --locked`. The store covers the exact third-party
package versions in `Cargo.lock`; an added dependency or version change fails
the gate until its audit path or explicit exemption is reviewed and committed.

The initial store uses `safe-to-deploy` exemptions for the existing dependency
baseline. An exemption is dependency-intake policy, **not evidence of a source
audit**. E2EE-specific exemptions retain the rationale from the former
inventory and explicitly state that formal source/security review is pending.
Audits recorded in `supply-chain/audits.toml` should replace exemptions over
time; Phase 7 still requires the formal OpenMLS review and external security
review.

**Formal security review of the OpenMLS dependency tree is tracked as a Phase 7
(Hardening and GA) exit criterion.** Until then, Cargo.lock pinning plus the
active audit/deny/vet gates are the implemented controls.

## Build Integrity

Per ADR 0001 and PLAN_E2EE.md:

- Code signing is necessary but not sufficient: it proves origin, not honesty.
- Signed releases with downgrade-protected update manifests are required for E2EE-capable clients.
- Reproducible builds enable third-party source-to-binary verification.
- Binary transparency for client releases is the roadmap endpoint.
- Build machines and signing-key custody remain disclosed trust dependencies.

## MPL-2.0 Binary Distribution

Every externally distributed executable form that contains the `hpke-rs`
family must give recipients notice and a reasonable way to obtain the exact
MPL-covered source. `THIRD_PARTY_NOTICES.txt` records the component versions,
license location, and immutable crates.io source-archive URLs.

- Tauri installers and signed updater artifacts bundle the notice as a resource.
- The server container copies the notice to `/usr/share/doc/filament/`.
- A raw desktop or server executable must be distributed with the same notice
  beside it, or through another mechanism that recipients reliably receive.
- If an MPL-covered source file is modified, the modified source—not merely the
  upstream archive—must be made available under MPL-2.0 and the notice updated.
- Filament files that contain no MPL-covered code keep their existing license.

## Verification Commands

```bash
# License, ban, and source gate
cargo deny --config cargo-deny.toml check

# Advisory gate
cargo audit

cargo vet --locked
```

## Spike Crate Exclusion

The spike crate at `spikes/e2ee-mls-roundtrip/` is a standalone crate and is
**not** part of the workspace. It has its own lockfile and is not listed in the
workspace members. Workspace-level `cargo deny` does not cover it. Its direct
OpenMLS versions match the production crate; it must be checked separately when
dependencies change.
