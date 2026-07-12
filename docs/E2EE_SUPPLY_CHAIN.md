# E2EE Supply Chain Documentation

**Phase 0 Deliverable:** Supply-chain gate configuration for OpenMLS dependencies.

## Overview

This document records the supply-chain review process for the E2EE dependency tree (OpenMLS and its transitive dependencies). It accompanies `cargo-deny.toml` (license/advisory/ban gates) and `cargo-vet.toml` (structured crate review).

## OpenMLS Dependency Tree

The primary new cryptographic dependency is **OpenMLS** (MIT licensed), an RFC 9420 MLS implementation in pure Rust.

### Direct Dependencies (added in Phase 1)

| Crate | Version | License | Purpose |
|---|---|---|---|
| `openmls` | 0.8.1 | MIT | Main MLS library — group state, KeyPackages, commits, messages |
| `openmls_traits` | 0.5.0 | MIT | Trait definitions for provider abstraction |
| `openmls_rust_crypto` | 0.5.1 | MIT | Default crypto provider (RustCrypto crates, no C deps) |
| `openmls_basic_credential` | 0.5.0 | MIT | BasicCredential + SignatureKeyPair for credential types |
| `openmls_memory_storage` | 0.5.0 | MIT | In-memory storage (spikes/tests only) |

### Key Transitive Dependencies

| Crate | License | Purpose | cargo-deny Status |
|---|---|---|---|
| `tls_codec` | MIT/Apache-2.0 | TLS serialization for MLS wire format | ✅ Passes |
| `zeroize` | Apache-2.0/MIT | Memory zeroization for key material | ✅ Passes |
| `serde` | MIT/Apache-2.0 | Serialization framework | ✅ Passes (already in workspace) |
| `serde_bytes` | MIT/Apache-2.0 | Byte array serde wrapper | ✅ Passes |
| `rayon` | MIT/Apache-2.0 | Parallelism for tree operations | ✅ Passes |
| `thiserror` | MIT/Apache-2.0 | Error derive macro | ✅ Passes (already in workspace) |
| `log` | MIT/Apache-2.0 | Logging facade | ✅ Passes |
| `getrandom` | MIT/Apache-2.0 | CSPRNG interface | ✅ Passes |
| RustCrypto crates (`chacha20poly1305`, `x25519-dalek`, `ed25519-dalek`, `sha2`, `hkdf`, `hpke`) | MIT/Apache-2.0 | Underlying cryptographic primitives | ✅ Passes |

All licenses are within the `cargo-deny.toml` allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib.

## cargo-deny Configuration

The existing `cargo-deny.toml` license allowlist already covers all OpenMLS dependencies (MIT is allowed). No license exceptions are needed for OpenMLS or its transitive dependencies.

Advisory exceptions: the existing `RUSTSEC-2024-0384` exception (for the `instant` crate transitively via Tantivy) is unrelated to OpenMLS. No new advisory exceptions are required for the OpenMLS dependency tree at the time of this writing.

## cargo-vet Configuration

A `cargo-vet.toml` file has been added to the repository root. It uses "unaudited" entries with documented justifications for each OpenMLS-related crate. This follows the pattern of recording a review process even when formal third-party audits are pending.

**Formal security review of the OpenMLS dependency tree is tracked as a Phase 7 (Hardening and GA) exit criterion.** The unaudited entries in `cargo-vet.toml` will be replaced with proper audit entries once the review is complete.

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

# Structured review gate (requires cargo-vet installed)
cargo vet --config cargo-vet.toml
```

## Spike Crate Exclusion

The spike crate at `spikes/e2ee-mls-roundtrip/` is a standalone crate and is **not** part of the workspace. It has its own `Cargo.toml` and is not listed in the workspace `members` array. Workspace-level `cargo deny` and `cargo vet` commands do not cover it. The spike crate's dependencies (openmls, openmls_rust_crypto, etc.) are the same versions that will be used in production (Phase 1) and have been verified against the cargo-deny license allowlist.
