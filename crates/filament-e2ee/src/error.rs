//! Error types for the filament-e2ee crate.
//!
//! All errors use `thiserror` and never leak key material in their `Display`
//! or `Debug` output.

use thiserror::Error;

/// Errors from identity and device certificate operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityError {
    /// Ed25519 key generation or signing failed.
    #[error("crypto operation failed")]
    CryptoError,
    /// Device certificate signature verification failed.
    #[error("device certificate signature verification failed")]
    SignatureVerificationFailed,
    /// Invalid input (bad user_id, device_id, key size, etc.).
    #[error("invalid identity input: {0}")]
    InvalidInput(String),
}

/// Errors from KeyPackage operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyPackageError {
    /// OpenMLS KeyPackage creation failed.
    #[error("keypackage creation failed")]
    CreationFailed,
    /// KeyPackage serialization or deserialization failed.
    #[error("keypackage serialization failed")]
    SerializationFailed,
    /// Pool size limit exceeded.
    #[error("keypackage pool size limit exceeded: max {max}, requested {requested}")]
    PoolLimitExceeded {
        /// Maximum allowed KeyPackages per device.
        max: usize,
        /// Number requested in this operation.
        requested: usize,
    },
    /// Pool is exhausted (no unclaimed KeyPackages remain).
    #[error("keypackage pool exhausted")]
    PoolExhausted,
}

/// Errors from the local key store.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyStoreError {
    /// Item not found in the store.
    #[error("keystore item not found: {0}")]
    NotFound(String),
    /// Store backend error.
    #[error("keystore backend error")]
    BackendError,
    /// Stored bytes do not match the expected type or length.
    #[error("keystore value is invalid")]
    InvalidValue,
}

/// Errors from short-lived QR device pairing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PairingError {
    /// The QR offer or encrypted transfer did not satisfy its wire invariants.
    #[error("invalid pairing payload")]
    InvalidPayload,
    /// The pairing offer is expired or outside the permitted lifetime.
    #[error("pairing offer expired")]
    Expired,
    /// The scanned device belongs to a different user.
    #[error("pairing user mismatch")]
    UserMismatch,
    /// A device attempted to pair with itself.
    #[error("pairing device mismatch")]
    DeviceMismatch,
    /// The returning transfer was not authenticated by the QR secret and sender device.
    #[error("pairing authentication failed")]
    AuthenticationFailed,
    /// A vetted provider operation failed without exposing key material.
    #[error("pairing crypto operation failed")]
    CryptoError,
    /// Strict pairing payload serialization or parsing failed.
    #[error("pairing serialization failed")]
    SerializationFailed,
}

/// Unified error type for the e2ee crate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum E2eeError {
    /// Identity / device certificate error.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// KeyPackage error.
    #[error(transparent)]
    KeyPackage(#[from] KeyPackageError),
    /// Local key store error.
    #[error(transparent)]
    KeyStore(#[from] KeyStoreError),
    /// Device pairing error.
    #[error(transparent)]
    Pairing(#[from] PairingError),
    /// OpenMLS internal error (opaque — no key material leaked).
    #[error("openmls error")]
    OpenMlsError,
}
