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
//! - **Mailbox**: fail-closed offline ciphertext processing and acknowledgment
//!   construction after successful MLS authentication.
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

pub mod application;
pub mod attachment;
pub mod backup;
pub mod commit_mailbox;
pub mod conversation;
pub mod delivery_service;
pub mod durable_mailbox;
pub mod error;
pub mod history_sync;
pub mod identity;
pub mod keypackage;
pub mod keystore;
pub mod mailbox;
pub mod pairing;
pub mod persistence;
#[cfg(feature = "sqlcipher-store")]
pub mod sqlcipher_store;

// Re-export the most commonly used types.
pub use application::{
    ApplicationEventId, AttachmentSet, ChatMessageBody, EncryptedAttachmentReference,
    EncryptedChatEvent, EncryptedMessageId, QuotePreview, ReactionAction, ReactionToken,
    ReplyReference, VersionedApplicationEvent, MAX_APPLICATION_EVENT_BYTES,
    MAX_ATTACHMENTS_PER_EVENT, MAX_CHAT_MESSAGE_BYTES, MAX_QUOTE_PREVIEW_BYTES, MAX_REACTION_CHARS,
};
pub use attachment::{
    decrypt_attachment, encrypt_attachment, encrypt_thumbnail, AttachmentContent,
    AttachmentDescriptor, AttachmentId, AttachmentKind, EncryptedAttachment,
    ATTACHMENT_CIPHERTEXT_BUCKETS, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_FILENAME_BYTES,
    MAX_ATTACHMENT_MIME_BYTES, MAX_ENCRYPTED_ATTACHMENT_BYTES, MAX_THUMBNAIL_BYTES,
};
pub use backup::{
    create_passphrase_backup, restore_passphrase_backup, BackupRestore, EncryptedBackup,
    ARGON2_BACKUP_ITERATIONS, ARGON2_BACKUP_MEMORY_KIB, MAX_BACKUP_BLOB_BYTES,
    MAX_BACKUP_PASSPHRASE_BYTES, MIN_BACKUP_PASSPHRASE_BYTES,
};
pub use commit_mailbox::{
    process_commit_mailbox, process_group_commit_mailbox, CommitMailboxBatch, RejectedMailboxCommit,
};
pub use conversation::{
    AuthenticatedMembershipChange, AuthenticatedMembershipChangeKind, ConversationAudience,
    DecryptedApplicationMessage, DecryptionOutcome, DeliveryServiceIdentity,
    EncryptedApplicationMessage, EncryptedGroupCommit, ExternalCommitRecoveryInfo,
    ExternalGroupProposal, ExternalProposalAction, MlsConversation, PendingCommitRebase,
    PendingGroupCommit, PinnedUserIdentity, MAX_APPLICATION_PLAINTEXT_BYTES,
    MAX_BUFFERED_GENERATION_GAP, MAX_MLS_DEVICES_PER_USER, MAX_MLS_GROUP_LEAVES,
    MAX_MLS_GROUP_USERS,
};
pub use delivery_service::{DeliveryServiceSigner, DELIVERY_SERVICE_SEED_BYTES};
pub use durable_mailbox::{
    confirm_commit_acknowledgment, confirm_message_acknowledgment, load_stored_message,
    pending_commit_acknowledgment, pending_message_acknowledgment, DurableCommitMailboxBatch,
    DurableMailboxError, DurableMessageMailboxBatch, DurableMlsClient, StoredMailboxMessage,
};
pub use error::{
    AttachmentError, BackupError, ConversationError, E2eeError, HistorySyncError, IdentityError,
    KeyPackageError, KeyStoreError, PairingError,
};
pub use history_sync::{
    EncryptedHistorySyncPage, HistorySyncImport, HistorySyncReceiver, HistorySyncSender,
    ScannedHistorySyncOffer, DEFAULT_HISTORY_SYNC_TTL_SECS, MAX_HISTORY_SYNC_OFFER_BYTES,
    MAX_HISTORY_SYNC_PAGE_BYTES, MAX_HISTORY_SYNC_TTL_SECS,
};
pub use identity::{
    create_root_identity_rotation_proof, safety_number, verify_device_certificate,
    verify_root_identity_rotation_chain, verify_root_identity_rotation_proof, RootIdentityKey,
    RootIdentityRotationProof,
};
pub use keypackage::{
    generate_key_package_batch, generate_last_resort_key_package, key_package_hash,
    GeneratedKeyPackage, KeyPackagePool, KeyPackagePoolEntry, MlsDevice, DEFAULT_BATCH_SIZE,
    DEFAULT_MAX_POOL_SIZE,
};
pub use keystore::{
    load_root_identity, persist_root_identity, InMemoryKeyStore, LocalKeyStore, LocalStoreId,
    StoreKey, StoreKeyProvider, MAX_STORE_BATCH_ENTRIES, MAX_STORE_ENTRIES, MAX_STORE_KEY_BYTES,
    MAX_STORE_VALUE_BYTES, STORE_ENCRYPTION_KEY_BYTES,
};
pub use mailbox::{
    process_message_mailbox, AuthenticatedMailboxMessage, MailboxDecryptionBatch,
    RejectedMailboxMessage,
};
pub use pairing::{
    create_pairing_transfer, PairedRootIdentity, PairingReceiver, PairingTransfer,
    ScannedPairingOffer, DEFAULT_PAIRING_TTL_SECS, MAX_PAIRING_OFFER_BYTES,
    MAX_PAIRING_TRANSFER_BYTES, MAX_PAIRING_TTL_SECS,
};
pub use persistence::{
    load_mls_client_state, persist_mls_client_state, MlsClientState, PendingExternalCommitRecovery,
};
#[cfg(feature = "sqlcipher-store")]
pub use sqlcipher_store::{SqlCipherKeyStore, MAX_ENCRYPTED_STORE_BYTES};
