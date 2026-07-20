# E2EE Supply Chain Documentation

**Phase 0 Deliverable:** Supply-chain gate configuration for OpenMLS dependencies.

## Overview

This document records the supply-chain review state for the E2EE dependency
tree (OpenMLS and its transitive dependencies). `cargo audit` and
`cargo deny check --config cargo-deny.toml` are enforced in CI.

## OpenMLS Dependency Tree

The primary new cryptographic dependency is **OpenMLS** (MIT licensed), an RFC 9420 MLS implementation in pure Rust.

### Direct Dependencies (added in Phase 1)

| Crate | Version | License | Purpose |
|---|---|---|---|
| `openmls` | 0.8.1 | MIT | Main MLS library — group state, KeyPackages, commits, messages |
| `openmls_rust_crypto` | 0.5.1 | MIT | Default crypto provider (RustCrypto crates, no C deps) |
| `openmls_basic_credential` | 0.5.0 | MIT | BasicCredential + SignatureKeyPair for credential types |
| `openmls_traits` | 0.5.0 | MIT | Direct access to the approved provider traits used by device pairing |

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
| RustCrypto crates (`chacha20poly1305`, `x25519-dalek`, `ed25519-dalek`, `sha2`, `hkdf`, `hpke`) | MIT/Apache-2.0 | Underlying cryptographic primitives | Allowed licenses |

All licenses are within the `cargo-deny.toml` allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib.

## cargo-deny Configuration

The existing `cargo-deny.toml` license allowlist already covers all OpenMLS dependencies (MIT is allowed). No license exceptions are needed for OpenMLS or its transitive dependencies.

Advisory exceptions: the existing `RUSTSEC-2024-0384` exception (for the `instant` crate transitively via Tantivy) is unrelated to OpenMLS. No new advisory exceptions are required for the OpenMLS dependency tree at the time of this writing.

## cargo-vet Status

The root `cargo-vet.toml` is an inventory of intended unaudited exceptions, not
an active cargo-vet store or CI gate. It must not be treated as evidence that
the dependency tree has been audited. Before cargo-vet can become enforceable,
the inventory must be converted to cargo-vet's generated store layout and the
CI workflow must install and run cargo-vet.

**Formal security review of the OpenMLS dependency tree is tracked as a Phase 7
(Hardening and GA) exit criterion.** Until then, Cargo.lock pinning plus the
active audit/deny gates are the implemented controls.

## Build Integrity

Per ADR 0001 and PLAN_E2EE.md:

- Code signing is necessary but not sufficient: it proves origin, not honesty.
- Signed releases with downgrade-protected update manifests are required for E2EE-capable clients.
- Reproducible builds enable third-party source-to-binary verification.
- Binary transparency for client releases is the roadmap endpoint.
- Build machines and signing-key custody remain disclosed trust dependencies.

## Verification Commands

```bash
# License, ban, and source gate
cargo deny check --config cargo-deny.toml

# Advisory gate
cargo audit

# cargo-vet is not yet an active gate; see "cargo-vet Status" above.
```

## Spike Crate Exclusion

The spike crate at `spikes/e2ee-mls-roundtrip/` is a standalone crate and is
**not** part of the workspace. It has its own lockfile and is not listed in the
workspace members. Workspace-level `cargo deny` does not cover it. Its direct
OpenMLS versions match the production crate; it must be checked separately when
dependencies change.
