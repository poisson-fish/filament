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
    #[error("keystore item not found")]
    NotFound,
    /// A store key or store identifier violated its domain invariants.
    #[error("keystore identifier is invalid")]
    InvalidIdentifier,
    /// Store backend error.
    #[error("keystore backend error")]
    BackendError,
    /// The OS credential store could not supply the database key.
    #[error("platform keystore is unavailable")]
    KeyUnavailable,
    /// The database path is not an absolute, regular, non-symlink file.
    #[error("encrypted store path is invalid")]
    InvalidPath,
    /// Stored bytes do not match the expected type or length.
    #[error("keystore value is invalid")]
    InvalidValue,
    /// A hard entry, value-size, or database-size limit was exceeded.
    #[error("keystore limit exceeded")]
    LimitExceeded,
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

/// Errors from authenticated device-to-device history synchronization.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HistorySyncError {
    /// An offer, page, or decrypted record violated its wire invariants.
    #[error("invalid history sync payload")]
    InvalidPayload,
    /// The short-lived receiving session expired.
    #[error("history sync session expired")]
    Expired,
    /// The two devices do not belong to the same account identity.
    #[error("history sync user mismatch")]
    UserMismatch,
    /// A device attempted to synchronize with itself or the wrong receiver.
    #[error("history sync device mismatch")]
    DeviceMismatch,
    /// A certificate, signature, HPKE context, or ciphertext was not authentic.
    #[error("history sync authentication failed")]
    AuthenticationFailed,
    /// A hard page, record, byte, or session limit was exceeded.
    #[error("history sync limit exceeded")]
    LimitExceeded,
    /// A page was replayed, skipped, or received after the terminal page.
    #[error("history sync page is out of order")]
    OutOfOrder,
    /// Imported history conflicts with an existing durable local record.
    #[error("history sync record conflicts with local history")]
    Conflict,
    /// The approved provider could not complete a history sync operation.
    #[error("history sync crypto operation failed")]
    CryptoError,
    /// Strict history sync serialization failed.
    #[error("history sync serialization failed")]
    SerializationFailed,
    /// Encrypted local persistence failed.
    #[error(transparent)]
    KeyStore(#[from] KeyStoreError),
}

/// Errors from encrypted attachment preparation and verification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AttachmentError {
    /// A filename, descriptor, MIME value, or ciphertext shape was invalid.
    #[error("invalid encrypted attachment")]
    InvalidAttachment,
    /// A plaintext or encrypted attachment exceeded a hard client limit.
    #[error("encrypted attachment limit exceeded")]
    LimitExceeded,
    /// The approved provider could not complete an attachment operation.
    #[error("encrypted attachment crypto operation failed")]
    CryptoError,
    /// Authenticated decryption or post-decryption content verification failed.
    #[error("encrypted attachment verification failed")]
    VerificationFailed,
}

/// Errors from an MLS conversation lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConversationError {
    /// A claimed `KeyPackage` was malformed, unverifiable, or for another suite.
    #[error("invalid keypackage")]
    InvalidKeyPackage,
    /// An MLS credential did not chain to a locally pinned root identity.
    #[error("untrusted MLS device credential")]
    UntrustedCredential,
    /// The group membership did not match the locally pinned audience policy.
    #[error("unexpected MLS group membership")]
    UnexpectedMembership,
    /// The MLS group identifier did not match the locally pinned conversation.
    #[error("MLS group identifier mismatch")]
    GroupMismatch,
    /// A server routing hint contradicted locally verified MLS state.
    #[error("untrusted routing metadata mismatch")]
    MetadataMismatch,
    /// An encrypted record was presented to a conversation not pinned to MLS v1.
    #[error("conversation crypto mode mismatch")]
    CryptoModeMismatch,
    /// An input exceeded a client-side hard limit.
    #[error("MLS conversation limit exceeded")]
    LimitExceeded,
    /// The application payload was malformed or was not an application message.
    #[error("invalid MLS application message")]
    InvalidApplicationMessage,
    /// A purported MLS commit was malformed or had the wrong content type.
    #[error("invalid MLS commit")]
    InvalidCommit,
    /// The application generation was already delivered.
    #[error("duplicate application generation")]
    DuplicateGeneration,
    /// The application generation is too far ahead to buffer safely.
    #[error("application generation gap exceeds limit")]
    GenerationGapExceeded,
    /// A mailbox page violated cursor, identifier, uniqueness, or size invariants.
    #[error("invalid encrypted mailbox page")]
    InvalidMailboxPage,
    /// The operation requires a pending local commit, but none exists.
    #[error("no pending MLS commit")]
    NoPendingCommit,
    /// The operation requires an operational group with no pending commit.
    #[error("MLS commit is pending delivery-service acceptance")]
    PendingCommit,
    /// The initial Add commit was not accepted, so the group is not sendable.
    #[error("MLS conversation is not active")]
    NotActive,
    /// A vetted OpenMLS operation failed without exposing key material.
    #[error("MLS conversation crypto operation failed")]
    CryptoError,
    /// Strict TLS or application-envelope serialization failed.
    #[error("MLS conversation serialization failed")]
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
    /// Device-to-device history synchronization error.
    #[error(transparent)]
    HistorySync(#[from] HistorySyncError),
    /// Encrypted attachment error.
    #[error(transparent)]
    Attachment(#[from] AttachmentError),
    /// MLS conversation lifecycle error.
    #[error(transparent)]
    Conversation(#[from] ConversationError),
    /// OpenMLS internal error (opaque — no key material leaked).
    #[error("openmls error")]
    OpenMlsError,
}
