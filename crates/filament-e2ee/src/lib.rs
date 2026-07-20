#![forbid(unsafe_code)]
#![allow(clippy::doc_markdown)]

//! Filament E2EE — MLS client core for end-to-end encryption.
//!
//! This crate provides the crypto foundation for all E2EE operations:
//!
//! - **Identity**: Root identity key generation (Ed25519), device certificate
//!   creation and verification.
//! - **KeyPackages**: MLS KeyPackage pool management (single-use + last-resort).
//! - **KeyStore**: Local encrypted store abstraction (trait + in-memory impl).
//! - **Pairing**: short-lived QR offers and authenticated HPKE root-key transfer.
//!
//! # Security Properties
//!
//! - `#![forbid(unsafe_code)]` is enforced at the crate level.
//! - All key material is zeroized on drop via the `zeroize` crate.
//! - No key material appears in `Debug`, `Display`, or error messages.
//! - The platform CSPRNG is the only source of randomness.
//! - The server never holds root keys, so it cannot mint valid device
//!   certificates — the ghost-device defense is cryptographic, not
//!   policy-based.
//!
//! See [`docs/adr/0001-e2ee-mls-openmls.md`] for the protocol decision and
//! [`plans/PLAN_E2EE.md`] for the full design specification.

pub mod error;
pub mod identity;
pub mod keypackage;
pub mod keystore;
pub mod pairing;
#[cfg(feature = "sqlcipher-store")]
pub mod sqlcipher_store;

// Re-export the most commonly used types.
pub use error::{E2eeError, IdentityError, KeyPackageError, KeyStoreError, PairingError};
pub use identity::{safety_number, verify_device_certificate, RootIdentityKey};
pub use keypackage::{
    generate_key_package_batch, generate_last_resort_key_package, key_package_hash,
    GeneratedKeyPackage, KeyPackagePool, KeyPackagePoolEntry, MlsDevice, DEFAULT_BATCH_SIZE,
    DEFAULT_MAX_POOL_SIZE,
};
pub use keystore::{
    load_root_identity, persist_root_identity, InMemoryKeyStore, LocalKeyStore, LocalStoreId,
    StoreKey, StoreKeyProvider, MAX_STORE_ENTRIES, MAX_STORE_KEY_BYTES, MAX_STORE_VALUE_BYTES,
    STORE_ENCRYPTION_KEY_BYTES,
};
pub use pairing::{
    create_pairing_transfer, PairedRootIdentity, PairingReceiver, PairingTransfer,
    ScannedPairingOffer, DEFAULT_PAIRING_TTL_SECS, MAX_PAIRING_OFFER_BYTES,
    MAX_PAIRING_TRANSFER_BYTES, MAX_PAIRING_TTL_SECS,
};
#[cfg(feature = "sqlcipher-store")]
pub use sqlcipher_store::{SqlCipherKeyStore, MAX_ENCRYPTED_STORE_BYTES};
