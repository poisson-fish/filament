//! Versioned, encrypted checkpoints for native MLS client state.
//!
//! OpenMLS persists group secrets into its provider as operations complete.
//! This module snapshots that complete provider together with Filament's
//! identity pins and generation queues into one [`LocalKeyStore`] record. A
//! mailbox acknowledgment is safe only after this checkpoint and any released
//! message history have both been durably written by the native runtime.

use std::collections::HashSet;

use filament_core::{DeviceCertificate, DeviceId, GroupId, UserId};
use filament_protocol::{
    RotateRootIdentityRequest, RotateRootIdentityResponse, MAX_KEYPACKAGE_BYTES,
    MAX_KEYPACKAGE_POOL_SIZE, MAX_ROOT_IDENTITY_ROTATIONS, ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
};
use openmls::prelude::{KeyPackageIn, ProtocolVersion};
use openmls_traits::OpenMlsProvider as _;
use serde::{Deserialize, Serialize};
use tls_codec::Deserialize as _;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    conversation::{ConversationPersistenceMetadata, InboundPersistenceMetadata},
    create_root_identity_rotation_proof,
    keypackage::{GeneratedKeyPackage, ProviderRecord},
    verify_root_identity_rotation_proof, ConversationAudience, DecryptedApplicationMessage,
    DeliveryServiceIdentity, ExternalCommitRecoveryInfo, KeyStoreError, LocalKeyStore,
    MlsConversation, MlsDevice, PendingGroupCommit, PinnedUserIdentity, RootIdentityKey,
    RootIdentityRotationProof, StoreKey, MAX_APPLICATION_PLAINTEXT_BYTES,
    MAX_BUFFERED_GENERATION_GAP, MAX_STORE_VALUE_BYTES,
};

const MLS_CLIENT_STATE_VERSION: u16 = 3;
const LEGACY_MLS_CLIENT_STATE_VERSIONS: [u16; 2] = [1, 2];
const MAX_MLS_CONVERSATIONS: usize = 1_024;
const MAX_OPENMLS_STORAGE_RECORDS: usize = 16_384;
const MAX_OPENMLS_STORAGE_KEY_BYTES: usize = 4_096;
const MAX_OPENMLS_STORAGE_RECORD_BYTES: usize = 1024 * 1024;
const PENDING_KEYPACKAGE_UPLOAD_VERSION: u16 = 1;
const PENDING_ROOT_ROTATION_VERSION: u16 = 1;
const ROTATION_SEQUENCE_BYTES: usize = size_of::<u64>();

/// Restart-safe native candidate for a destructive account-root rotation.
///
/// The replacement root, device signer, and KeyPackage private material are
/// loaded only from the encrypted local store. Public wire fields can be
/// submitted repeatedly until the Delivery Service confirms the exact
/// transition.
pub struct PendingRootIdentityRotation {
    request: RotateRootIdentityRequest,
    previous_root_key_pub: [u8; 32],
    replacement_root: RootIdentityKey,
    replacement_device: MlsDevice,
    checkpoint: Zeroizing<Vec<u8>>,
    pending_keypackages: Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for PendingRootIdentityRotation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingRootIdentityRotation")
            .field(
                "expected_rotation_sequence",
                &self.request.expected_rotation_sequence,
            )
            .field("device_id", &self.request.device_id)
            .field("secret_state", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl PendingRootIdentityRotation {
    /// Return the public, dual-signed transition for bounded native transport.
    #[must_use]
    pub const fn wire_request(&self) -> &RotateRootIdentityRequest {
        &self.request
    }

    /// Atomically adopt the confirmed replacement identity and fresh MLS
    /// provider, then remove the encrypted retry record.
    ///
    /// Existing MLS groups are intentionally not copied: root rotation is a
    /// destructive recovery boundary and the replacement device must recover
    /// conversations through authenticated external commits.
    ///
    /// # Errors
    /// Rejects any response that differs from the durable candidate, or a
    /// local-store failure before the replacement checkpoint is durable.
    pub fn finish(
        self,
        response: &RotateRootIdentityResponse,
        store: &dyn LocalKeyStore,
    ) -> Result<MlsDevice, KeyStoreError> {
        let next_sequence = self
            .request
            .expected_rotation_sequence
            .checked_add(1)
            .ok_or(KeyStoreError::InvalidValue)?;
        if response.protocol_version != ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION
            || response.user_id != self.replacement_device.user_id().to_string()
            || response.device_id != self.replacement_device.device_id().to_string()
            || response.rotation_sequence != next_sequence
            || response.previous_root_key_pub != self.previous_root_key_pub
            || response.new_root_key_pub != self.replacement_root.public_key_bytes()
            || !(0..=253_402_300_799).contains(&response.rotated_at_unix)
        {
            return Err(KeyStoreError::InvalidValue);
        }
        let root_secret = self.replacement_root.secret_bytes();
        store.store_batch(vec![
            (StoreKey::root_identity(), root_secret.to_vec()),
            (StoreKey::mls_client_state(), self.checkpoint.to_vec()),
            (
                StoreKey::pending_keypackage_upload(),
                self.pending_keypackages.to_vec(),
            ),
            (
                StoreKey::root_identity_rotation_sequence(),
                next_sequence.to_be_bytes().to_vec(),
            ),
        ])?;
        let removed = store.remove_batch(&[StoreKey::pending_root_identity_rotation()])?;
        if removed != 1 {
            return Err(KeyStoreError::BackendError);
        }
        Ok(self.replacement_device)
    }
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedPendingRootIdentityRotation {
    version: u16,
    expected_rotation_sequence: u64,
    user_id: String,
    device_id: String,
    previous_root_key_pub: Vec<u8>,
    replacement_root_secret: Vec<u8>,
    new_root_key_pub: Vec<u8>,
    previous_root_signature: Vec<u8>,
    new_root_signature: Vec<u8>,
    new_device_signature_pubkey: Vec<u8>,
    new_device_root_signature: Vec<u8>,
    checkpoint: Vec<u8>,
    pending_keypackages: Vec<u8>,
}

/// One opaque public KeyPackage retained until the Delivery Service confirms
/// its idempotent upload.
#[derive(Clone, PartialEq, Eq)]
pub struct PendingKeyPackage {
    pub blob: Vec<u8>,
    pub is_last_resort: bool,
}

impl core::fmt::Debug for PendingKeyPackage {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingKeyPackage")
            .field("blob_bytes", &self.blob.len())
            .field("is_last_resort", &self.is_last_resort)
            .finish()
    }
}

/// Bounded native-only upload outbox for freshly generated KeyPackages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingKeyPackageUpload {
    pub packages: Vec<PendingKeyPackage>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedPendingKeyPackageUpload {
    version: u16,
    packages: Vec<PersistedPendingKeyPackage>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedPendingKeyPackage {
    blob: Vec<u8>,
    is_last_resort: bool,
}

/// Restored native-only MLS state for one certified device.
pub struct MlsClientState {
    pub device: MlsDevice,
    pub conversations: Vec<MlsConversation>,
}

/// Isolated candidate state for one acceptance-gated external commit.
///
/// Building an external commit writes group secrets into the OpenMLS
/// provider. This type owns a clone of the complete native checkpoint so a
/// rejected Delivery Service write cannot alter the currently usable state.
pub struct PendingExternalCommitRecovery {
    state: MlsClientState,
    commit: PendingGroupCommit,
    base_checkpoint: Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for PendingExternalCommitRecovery {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingExternalCommitRecovery")
            .field("group_id", &self.commit.group_id)
            .field("base_checkpoint", &"<MLS key material omitted>")
            .field("recovery_epoch", &self.commit.epoch)
            .field("state", &"<MLS key material omitted>")
            .finish()
    }
}

impl PendingExternalCommitRecovery {
    /// Opaque commit material to submit to the Delivery Service.
    #[must_use]
    pub const fn pending_commit(&self) -> &PendingGroupCommit {
        &self.commit
    }

    pub(crate) fn base_checkpoint(&self) -> &[u8] {
        &self.base_checkpoint
    }

    pub(crate) fn into_state(self) -> MlsClientState {
        self.state
    }
}

impl core::fmt::Debug for MlsClientState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MlsClientState")
            .field("device_id", &self.device.device_id())
            .field("conversation_count", &self.conversations.len())
            .field("state", &"<MLS key material omitted>")
            .finish()
    }
}

impl MlsClientState {
    /// Build an external-commit recovery against an isolated checkpoint.
    ///
    /// The current state is not mutated. Callers submit [`PendingGroupCommit`]
    /// to the Delivery Service and adopt the candidate only after an exact
    /// acceptance response and durable persistence.
    ///
    /// # Errors
    /// Returns a fail-closed conversation error for stale/mismatched routing
    /// hints, untrusted `GroupInfo`, invalid membership, or MLS failure.
    pub fn prepare_external_commit_recovery(
        &self,
        peer: PinnedUserIdentity,
        recovery: &ExternalCommitRecoveryInfo,
    ) -> Result<PendingExternalCommitRecovery, crate::ConversationError> {
        let current = self
            .conversations
            .iter()
            .find(|conversation| conversation.group_id() == recovery.group_id)
            .ok_or(crate::ConversationError::GroupMismatch)?;
        let metadata = current.persistence_metadata();
        let peer_pin_matches = metadata.audience == ConversationAudience::DirectMessage
            && peer.user_id != self.device.user_id()
            && metadata.pinned_roots.len() == 2
            && metadata.pinned_roots.iter().any(|(user_id, root_key)| {
                *user_id == peer.user_id && *root_key == peer.root_key_pub
            });
        if !peer_pin_matches {
            return Err(crate::ConversationError::MetadataMismatch);
        }
        self.prepare_external_commit_recovery_candidate(recovery)
    }

    /// Build an isolated external-commit recovery for a bounded group DM.
    ///
    /// `participants` must be the exact locally pinned set of other root
    /// identities. Requiring the caller's current trust view prevents a
    /// server-supplied `GroupInfo` from silently changing the recovery
    /// audience before MLS authentication and membership validation run.
    ///
    /// # Errors
    /// Rejects incomplete, duplicate, conflicting, or non-group participant
    /// pins and otherwise applies the same stale-state and MLS checks as
    /// [`Self::prepare_external_commit_recovery`].
    pub fn prepare_group_external_commit_recovery(
        &self,
        participants: &[PinnedUserIdentity],
        recovery: &ExternalCommitRecoveryInfo,
    ) -> Result<PendingExternalCommitRecovery, crate::ConversationError> {
        let current = self
            .conversations
            .iter()
            .find(|conversation| conversation.group_id() == recovery.group_id)
            .ok_or(crate::ConversationError::GroupMismatch)?;
        let metadata = current.persistence_metadata();
        if metadata.audience != ConversationAudience::GroupDm
            || participants.len() != metadata.pinned_roots.len().saturating_sub(1)
        {
            return Err(crate::ConversationError::MetadataMismatch);
        }

        let own_user_id = self.device.user_id();
        let mut expected = std::collections::HashMap::with_capacity(participants.len());
        for participant in participants {
            if participant.user_id == own_user_id
                || expected
                    .insert(participant.user_id, participant.root_key_pub)
                    .is_some()
            {
                return Err(crate::ConversationError::MetadataMismatch);
            }
        }
        if metadata.pinned_roots.iter().any(|(user_id, root_key)| {
            *user_id != own_user_id && expected.get(user_id) != Some(root_key)
        }) {
            return Err(crate::ConversationError::MetadataMismatch);
        }

        self.prepare_external_commit_recovery_candidate(recovery)
    }

    fn prepare_external_commit_recovery_candidate(
        &self,
        recovery: &ExternalCommitRecoveryInfo,
    ) -> Result<PendingExternalCommitRecovery, crate::ConversationError> {
        let current = self
            .conversations
            .iter()
            .find(|conversation| conversation.group_id() == recovery.group_id)
            .ok_or(crate::ConversationError::GroupMismatch)?;
        let metadata = current.persistence_metadata();
        if (metadata.active && recovery.epoch <= metadata.epoch)
            || (!metadata.active && recovery.epoch < metadata.epoch)
        {
            return Err(crate::ConversationError::MetadataMismatch);
        }

        let conversations = self.conversations.iter().collect::<Vec<_>>();
        let base_checkpoint = encode_mls_client_state(&self.device, &conversations)
            .map_err(|_| crate::ConversationError::CryptoError)?;
        let mut candidate = clone_client_state(&base_checkpoint)
            .map_err(|_| crate::ConversationError::CryptoError)?;
        let position = candidate
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == recovery.group_id)
            .ok_or(crate::ConversationError::GroupMismatch)?;
        let current = candidate.conversations.remove(position);
        let (conversation, commit) =
            current.recover_by_external_commit(recovery, &candidate.device)?;
        candidate.conversations.insert(position, conversation);
        Ok(PendingExternalCommitRecovery {
            state: candidate,
            commit,
            base_checkpoint,
        })
    }
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedClientState {
    version: u16,
    device: PersistedDevice,
    provider_records: Vec<PersistedProviderRecord>,
    conversations: Vec<PersistedConversation>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedDevice {
    user_id: String,
    device_id: String,
    device_signature_pubkey: Vec<u8>,
    root_key_signature: Vec<u8>,
    root_key_pub: Vec<u8>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedProviderRecord {
    key: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedConversation {
    group_id: String,
    epoch: u64,
    own_device_id: String,
    #[serde(default)]
    audience: PersistedAudience,
    pinned_roots: Vec<PersistedRootPin>,
    #[serde(default)]
    delivery_service_signature_key: Option<Vec<u8>>,
    outbound_generation: u64,
    inbound: Vec<PersistedInboundQueue>,
    active: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize, Zeroize, Default)]
#[serde(rename_all = "snake_case")]
enum PersistedAudience {
    #[default]
    DirectMessage,
    GroupDm,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedRootPin {
    user_id: String,
    root_key_pub: Vec<u8>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedInboundQueue {
    device_id: String,
    next_generation: u64,
    pending: Vec<PersistedMessage>,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct PersistedMessage {
    sender_user_id: String,
    sender_device_id: String,
    generation: u64,
    plaintext: Vec<u8>,
}

/// Atomically persist the complete MLS provider and Filament conversation state.
///
/// `conversations` must contain every active conversation owned by `device`.
/// The backend's single-record upsert is the durability boundary. Callers must
/// not send mailbox acknowledgments until this succeeds and released message
/// history is durably stored.
///
/// # Errors
/// Returns [`KeyStoreError`] for duplicate/mismatched conversations, provider
/// failures, serialization failures, or any hard state-size limit.
pub fn persist_mls_client_state(
    store: &dyn LocalKeyStore,
    device: &MlsDevice,
    conversations: &[&MlsConversation],
) -> Result<(), KeyStoreError> {
    let encoded = encode_mls_client_state(device, conversations)?;
    store.store(StoreKey::mls_client_state(), encoded.to_vec())
}

/// Create and durably retain a fresh destructive root-rotation candidate.
///
/// The encrypted retry record is written before callers may submit the public
/// request. An existing candidate is never overwritten with new randomness;
/// callers must load and reconcile it instead.
///
/// # Errors
/// Returns a validation, generation, serialization, or encrypted-store error.
pub fn prepare_pending_root_identity_rotation(
    store: &dyn LocalKeyStore,
    user_id: UserId,
    device_id: DeviceId,
    expected_rotation_sequence: u64,
    previous_root: &RootIdentityKey,
) -> Result<PendingRootIdentityRotation, KeyStoreError> {
    if store.exists(&StoreKey::pending_root_identity_rotation())? {
        return Err(KeyStoreError::InvalidValue);
    }
    let next_sequence = expected_rotation_sequence
        .checked_add(1)
        .filter(|sequence| {
            usize::try_from(*sequence).is_ok_and(|sequence| sequence <= MAX_ROOT_IDENTITY_ROTATIONS)
        })
        .ok_or(KeyStoreError::LimitExceeded)?;
    let replacement_root = RootIdentityKey::generate();
    let proof = create_root_identity_rotation_proof(
        previous_root,
        &replacement_root,
        user_id,
        next_sequence,
    )
    .map_err(|_| KeyStoreError::InvalidValue)?;
    let replacement_device = MlsDevice::generate(user_id, device_id, &replacement_root)
        .map_err(|_| KeyStoreError::BackendError)?;
    let mut packages =
        crate::generate_key_package_batch(&replacement_device, crate::DEFAULT_BATCH_SIZE)
            .map_err(|_| KeyStoreError::BackendError)?;
    packages.push(
        crate::generate_last_resort_key_package(&replacement_device)
            .map_err(|_| KeyStoreError::BackendError)?,
    );
    let checkpoint = encode_mls_client_state(&replacement_device, &[])?;
    let pending_keypackages = encode_pending_keypackages(&packages)?;
    let certificate = replacement_device.certificate();
    let mut persisted = PersistedPendingRootIdentityRotation {
        version: PENDING_ROOT_ROTATION_VERSION,
        expected_rotation_sequence,
        user_id: user_id.to_string(),
        device_id: device_id.to_string(),
        previous_root_key_pub: proof.previous_root_key_pub.to_vec(),
        replacement_root_secret: replacement_root.secret_bytes().to_vec(),
        new_root_key_pub: proof.new_root_key_pub.to_vec(),
        previous_root_signature: proof.previous_root_signature.to_vec(),
        new_root_signature: proof.new_root_signature.to_vec(),
        new_device_signature_pubkey: certificate.device_signature_pubkey.clone(),
        new_device_root_signature: certificate.root_key_signature.clone(),
        checkpoint: checkpoint.to_vec(),
        pending_keypackages: pending_keypackages.to_vec(),
    };
    let encoded = serde_json::to_vec(&persisted).map_err(|_| KeyStoreError::BackendError);
    persisted.zeroize();
    let encoded = Zeroizing::new(encoded?);
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    store.store_batch_if_absent_or_equal(vec![(
        StoreKey::pending_root_identity_rotation(),
        encoded.to_vec(),
    )])?;
    decode_pending_root_identity_rotation(&encoded)
}

/// Restore and fully revalidate a pending destructive root rotation.
///
/// # Errors
/// Rejects missing, corrupt, oversized, or internally inconsistent state.
pub fn load_pending_root_identity_rotation(
    store: &dyn LocalKeyStore,
) -> Result<PendingRootIdentityRotation, KeyStoreError> {
    let encoded = store.load(&StoreKey::pending_root_identity_rotation())?;
    decode_pending_root_identity_rotation(&encoded)
}

/// Persist the last authenticated root-rotation sequence.
///
/// # Errors
/// Returns a limit or encrypted-store backend error.
pub fn persist_root_identity_rotation_sequence(
    store: &dyn LocalKeyStore,
    sequence: u64,
) -> Result<(), KeyStoreError> {
    if !usize::try_from(sequence).is_ok_and(|sequence| sequence <= MAX_ROOT_IDENTITY_ROTATIONS) {
        return Err(KeyStoreError::LimitExceeded);
    }
    store.store(
        StoreKey::root_identity_rotation_sequence(),
        sequence.to_be_bytes().to_vec(),
    )
}

/// Load the last authenticated root-rotation sequence.
///
/// # Errors
/// Rejects missing, malformed, or out-of-range records.
pub fn load_root_identity_rotation_sequence(
    store: &dyn LocalKeyStore,
) -> Result<u64, KeyStoreError> {
    let encoded = store.load(&StoreKey::root_identity_rotation_sequence())?;
    let bytes: [u8; ROTATION_SEQUENCE_BYTES] = encoded
        .as_slice()
        .try_into()
        .map_err(|_| KeyStoreError::InvalidValue)?;
    let sequence = u64::from_be_bytes(bytes);
    if !usize::try_from(sequence).is_ok_and(|sequence| sequence <= MAX_ROOT_IDENTITY_ROTATIONS) {
        return Err(KeyStoreError::InvalidValue);
    }
    Ok(sequence)
}

/// Atomically create a new device's root identity, complete MLS provider
/// checkpoint, and retryable KeyPackage upload outbox.
///
/// Existing exact records make this operation idempotent. Any conflicting
/// record fails closed without overwriting the previously enrolled identity.
///
/// # Errors
/// Returns [`KeyStoreError`] for mismatched identity material, invalid package
/// bounds, serialization failures, or an encrypted-store conflict.
pub fn persist_initial_device_bootstrap(
    store: &dyn LocalKeyStore,
    root_identity: &RootIdentityKey,
    device: &MlsDevice,
    packages: &[GeneratedKeyPackage],
) -> Result<(), KeyStoreError> {
    if device.root_key_public() != &root_identity.public_key_bytes()
        || packages
            .iter()
            .filter(|package| package.is_last_resort)
            .count()
            != 1
    {
        return Err(KeyStoreError::InvalidValue);
    }
    let expected_credential = device.credential_with_key();
    if packages.iter().any(|package| {
        package.key_package().leaf_node().credential() != &expected_credential.credential
            || package.key_package().leaf_node().signature_key()
                != &expected_credential.signature_key
    }) {
        return Err(KeyStoreError::InvalidValue);
    }
    let checkpoint = encode_mls_client_state(device, &[])?;
    let pending = encode_pending_keypackages(packages)?;
    let root_secret = root_identity.secret_bytes();
    store.store_batch_if_absent_or_equal(vec![
        (StoreKey::root_identity(), root_secret.to_vec()),
        (StoreKey::mls_client_state(), checkpoint.to_vec()),
        (StoreKey::pending_keypackage_upload(), pending.to_vec()),
        (
            StoreKey::root_identity_rotation_sequence(),
            0_u64.to_be_bytes().to_vec(),
        ),
    ])?;
    Ok(())
}

fn decode_pending_root_identity_rotation(
    encoded: &[u8],
) -> Result<PendingRootIdentityRotation, KeyStoreError> {
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut persisted: PersistedPendingRootIdentityRotation =
        serde_json::from_slice(encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let result = (|| {
        let user_id =
            UserId::try_from(persisted.user_id.clone()).map_err(|_| KeyStoreError::InvalidValue)?;
        let device_id = DeviceId::try_from(persisted.device_id.clone())
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let next_sequence = persisted
            .expected_rotation_sequence
            .checked_add(1)
            .filter(|sequence| {
                usize::try_from(*sequence)
                    .is_ok_and(|sequence| sequence <= MAX_ROOT_IDENTITY_ROTATIONS)
            })
            .ok_or(KeyStoreError::InvalidValue)?;
        if persisted.version != PENDING_ROOT_ROTATION_VERSION
            || persisted.replacement_root_secret.len() != 32
        {
            return Err(KeyStoreError::InvalidValue);
        }
        let proof = pending_rotation_proof(&persisted, next_sequence)?;
        verify_root_identity_rotation_proof(user_id, &proof)
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let replacement_secret: [u8; 32] = persisted
            .replacement_root_secret
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let replacement_secret = Zeroizing::new(replacement_secret);
        let replacement_root = RootIdentityKey::from_secret_bytes(&replacement_secret);
        if replacement_root.public_key_bytes() != proof.new_root_key_pub {
            return Err(KeyStoreError::InvalidValue);
        }
        let replacement_state = decode_mls_client_state(&persisted.checkpoint)?;
        if !replacement_state.conversations.is_empty()
            || replacement_state.device.user_id() != user_id
            || replacement_state.device.device_id() != device_id
            || replacement_state.device.root_key_public() != &proof.new_root_key_pub
            || replacement_state
                .device
                .certificate()
                .device_signature_pubkey
                != persisted.new_device_signature_pubkey
            || replacement_state.device.certificate().root_key_signature
                != persisted.new_device_root_signature
        {
            return Err(KeyStoreError::InvalidValue);
        }
        validate_keypackage_upload_for_device(
            &persisted.pending_keypackages,
            &replacement_state.device,
        )?;
        let request = RotateRootIdentityRequest {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            expected_rotation_sequence: persisted.expected_rotation_sequence,
            device_id: device_id.to_string(),
            new_root_key_pub: proof.new_root_key_pub.to_vec(),
            previous_root_signature: proof.previous_root_signature.to_vec(),
            new_root_signature: proof.new_root_signature.to_vec(),
            new_device_signature_pubkey: persisted.new_device_signature_pubkey.clone(),
            new_device_root_signature: persisted.new_device_root_signature.clone(),
        };
        Ok(PendingRootIdentityRotation {
            request,
            previous_root_key_pub: proof.previous_root_key_pub,
            replacement_root,
            replacement_device: replacement_state.device,
            checkpoint: Zeroizing::new(persisted.checkpoint.clone()),
            pending_keypackages: Zeroizing::new(persisted.pending_keypackages.clone()),
        })
    })();
    persisted.zeroize();
    result
}

fn pending_rotation_proof(
    persisted: &PersistedPendingRootIdentityRotation,
    sequence: u64,
) -> Result<RootIdentityRotationProof, KeyStoreError> {
    Ok(RootIdentityRotationProof {
        sequence,
        previous_root_key_pub: persisted
            .previous_root_key_pub
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?,
        new_root_key_pub: persisted
            .new_root_key_pub
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?,
        previous_root_signature: persisted
            .previous_root_signature
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?,
        new_root_signature: persisted
            .new_root_signature
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::InvalidValue)?,
    })
}

pub(crate) fn decode_mls_client_state(encoded: &[u8]) -> Result<MlsClientState, KeyStoreError> {
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut snapshot: PersistedClientState =
        serde_json::from_slice(encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let restored = restore_snapshot(&snapshot);
    snapshot.zeroize();
    restored
}

fn validate_keypackage_upload_for_device(
    encoded: &[u8],
    device: &MlsDevice,
) -> Result<(), KeyStoreError> {
    let mut persisted: PersistedPendingKeyPackageUpload =
        serde_json::from_slice(encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let result = (|| {
        validate_persisted_pending_keypackages(&persisted)?;
        let expected = device.credential_with_key();
        for package in &persisted.packages {
            let mut bytes = package.blob.as_slice();
            let incoming = KeyPackageIn::tls_deserialize(&mut bytes)
                .map_err(|_| KeyStoreError::InvalidValue)?;
            if !bytes.is_empty() {
                return Err(KeyStoreError::InvalidValue);
            }
            let validated = incoming
                .validate(device.provider().crypto(), ProtocolVersion::Mls10)
                .map_err(|_| KeyStoreError::InvalidValue)?;
            if validated.ciphersuite()
                != openmls::prelude::Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519
                || validated.leaf_node().credential() != &expected.credential
                || validated.leaf_node().signature_key() != &expected.signature_key
            {
                return Err(KeyStoreError::InvalidValue);
            }
        }
        Ok(())
    })();
    persisted.zeroize();
    result
}

/// Load and revalidate the exact KeyPackage upload outbox after restart.
///
/// # Errors
/// Returns [`KeyStoreError`] if the record is missing, corrupt, oversized, or
/// violates the at-most-one last-resort package invariant. Initial bootstrap
/// separately requires exactly one fallback; ordinary replenishment carries
/// none because a low-water event cannot prove fallback exhaustion.
pub fn load_pending_keypackage_upload(
    store: &dyn LocalKeyStore,
) -> Result<PendingKeyPackageUpload, KeyStoreError> {
    let encoded = store.load(&StoreKey::pending_keypackage_upload())?;
    let mut persisted: PersistedPendingKeyPackageUpload =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let result =
        validate_persisted_pending_keypackages(&persisted).map(|()| PendingKeyPackageUpload {
            packages: persisted
                .packages
                .iter()
                .map(|package| PendingKeyPackage {
                    blob: package.blob.clone(),
                    is_last_resort: package.is_last_resort,
                })
                .collect(),
        });
    persisted.zeroize();
    result
}

/// Remove a confirmed KeyPackage upload outbox. Missing state is accepted so
/// a repeated confirmed response is harmless.
///
/// # Errors
/// Returns an opaque backend error if the encrypted store cannot be queried or
/// updated.
pub fn clear_pending_keypackage_upload(store: &dyn LocalKeyStore) -> Result<(), KeyStoreError> {
    let key = StoreKey::pending_keypackage_upload();
    if store.exists(&key)? {
        store.remove(&key)?;
    }
    Ok(())
}

pub(crate) fn encode_pending_keypackages(
    packages: &[GeneratedKeyPackage],
) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
    let mut persisted = PersistedPendingKeyPackageUpload {
        version: PENDING_KEYPACKAGE_UPLOAD_VERSION,
        packages: packages
            .iter()
            .map(|package| PersistedPendingKeyPackage {
                blob: package.blob.clone(),
                is_last_resort: package.is_last_resort,
            })
            .collect(),
    };
    validate_persisted_pending_keypackages(&persisted)?;
    let encoded = serde_json::to_vec(&persisted)
        .map(Zeroizing::new)
        .map_err(|_| KeyStoreError::BackendError);
    persisted.zeroize();
    let encoded = encoded?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    Ok(encoded)
}

fn validate_persisted_pending_keypackages(
    pending: &PersistedPendingKeyPackageUpload,
) -> Result<(), KeyStoreError> {
    let unique_blobs = pending
        .packages
        .iter()
        .map(|package| package.blob.as_slice())
        .collect::<HashSet<_>>();
    if pending.version != PENDING_KEYPACKAGE_UPLOAD_VERSION
        || pending.packages.is_empty()
        || pending.packages.len() > MAX_KEYPACKAGE_POOL_SIZE
        || unique_blobs.len() != pending.packages.len()
        || pending
            .packages
            .iter()
            .any(|package| package.blob.is_empty() || package.blob.len() > MAX_KEYPACKAGE_BYTES)
        || pending
            .packages
            .iter()
            .filter(|package| package.is_last_resort)
            .count()
            > 1
    {
        return Err(KeyStoreError::InvalidValue);
    }
    Ok(())
}

pub(crate) fn encode_mls_client_state(
    device: &MlsDevice,
    conversations: &[&MlsConversation],
) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
    if conversations.len() > MAX_MLS_CONVERSATIONS {
        return Err(KeyStoreError::LimitExceeded);
    }
    let mut records = device.provider_records()?;
    let encoded = encode_snapshot(device, conversations, &records);
    zeroize_provider_records(&mut records);
    encoded
}

fn encode_snapshot(
    device: &MlsDevice,
    conversations: &[&MlsConversation],
    records: &[ProviderRecord],
) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
    validate_provider_records(records)?;
    let mut group_ids = HashSet::with_capacity(conversations.len());
    let mut persisted_conversations = Vec::with_capacity(conversations.len());
    for conversation in conversations {
        let metadata = conversation.persistence_metadata();
        if metadata.own_device_id != device.device_id() || !group_ids.insert(metadata.group_id) {
            return Err(KeyStoreError::InvalidValue);
        }
        persisted_conversations.push(PersistedConversation::from_metadata(metadata));
    }
    persisted_conversations.sort_by(|left, right| left.group_id.cmp(&right.group_id));
    let certificate = device.certificate();
    let mut snapshot = PersistedClientState {
        version: MLS_CLIENT_STATE_VERSION,
        device: PersistedDevice {
            user_id: certificate.user_id.clone(),
            device_id: certificate.device_id.clone(),
            device_signature_pubkey: certificate.device_signature_pubkey.clone(),
            root_key_signature: certificate.root_key_signature.clone(),
            root_key_pub: device.root_key_public().to_vec(),
        },
        provider_records: records
            .iter()
            .map(|(key, value)| PersistedProviderRecord {
                key: key.clone(),
                value: value.clone(),
            })
            .collect(),
        conversations: persisted_conversations,
    };
    let encoded = serde_json::to_vec(&snapshot).map(Zeroizing::new);
    snapshot.zeroize();
    let encoded = encoded.map_err(|_| KeyStoreError::BackendError)?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    Ok(encoded)
}

/// Restore a complete MLS device and its conversations from encrypted storage.
///
/// All identifiers, certificates, provider-record bounds, epochs, pins, group
/// memberships, and generation queues are revalidated before state is returned.
///
/// # Errors
/// Returns [`KeyStoreError::InvalidValue`] for a corrupt, mixed, rolled-back,
/// or unsupported checkpoint. Backend errors remain opaque.
pub fn load_mls_client_state(store: &dyn LocalKeyStore) -> Result<MlsClientState, KeyStoreError> {
    let encoded = store.load(&StoreKey::mls_client_state())?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut snapshot: PersistedClientState =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let restored = restore_snapshot(&snapshot);
    snapshot.zeroize();
    restored
}

fn restore_snapshot(snapshot: &PersistedClientState) -> Result<MlsClientState, KeyStoreError> {
    if !(LEGACY_MLS_CLIENT_STATE_VERSIONS.contains(&snapshot.version)
        || snapshot.version == MLS_CLIENT_STATE_VERSION)
        || snapshot.conversations.len() > MAX_MLS_CONVERSATIONS
        || snapshot.device.root_key_pub.len() != 32
    {
        return Err(KeyStoreError::InvalidValue);
    }
    let certificate = DeviceCertificate::try_new(
        snapshot.device.user_id.clone(),
        snapshot.device.device_id.clone(),
        snapshot.device.device_signature_pubkey.clone(),
        snapshot.device.root_key_signature.clone(),
    )
    .map_err(|_| KeyStoreError::InvalidValue)?;
    let root_key_pub: [u8; 32] = snapshot
        .device
        .root_key_pub
        .as_slice()
        .try_into()
        .map_err(|_| KeyStoreError::InvalidValue)?;
    let mut records = snapshot
        .provider_records
        .iter()
        .map(|record| (record.key.clone(), record.value.clone()))
        .collect::<Vec<_>>();
    if validate_provider_records(&records).is_err() {
        zeroize_provider_records(&mut records);
        return Err(KeyStoreError::InvalidValue);
    }
    let device = MlsDevice::restore(certificate, root_key_pub, &records);
    zeroize_provider_records(&mut records);
    let device = device?;

    let mut group_ids = HashSet::with_capacity(snapshot.conversations.len());
    let mut conversations = Vec::with_capacity(snapshot.conversations.len());
    for persisted in &snapshot.conversations {
        if snapshot.version == LEGACY_MLS_CLIENT_STATE_VERSIONS[0]
            && !matches!(persisted.audience, PersistedAudience::DirectMessage)
        {
            return Err(KeyStoreError::InvalidValue);
        }
        let metadata = persisted.to_metadata()?;
        if !group_ids.insert(metadata.group_id) {
            return Err(KeyStoreError::InvalidValue);
        }
        conversations.push(
            MlsConversation::restore(&device, metadata).map_err(|_| KeyStoreError::InvalidValue)?,
        );
    }
    Ok(MlsClientState {
        device,
        conversations,
    })
}

pub(crate) fn clone_client_state(encoded: &[u8]) -> Result<MlsClientState, KeyStoreError> {
    decode_mls_client_state(encoded)
}

fn validate_provider_records(records: &[ProviderRecord]) -> Result<(), KeyStoreError> {
    if records.is_empty() || records.len() > MAX_OPENMLS_STORAGE_RECORDS {
        return Err(KeyStoreError::LimitExceeded);
    }
    let mut keys = HashSet::with_capacity(records.len());
    let mut total = 0_usize;
    for (key, value) in records {
        if key.is_empty()
            || key.len() > MAX_OPENMLS_STORAGE_KEY_BYTES
            || value.is_empty()
            || value.len() > MAX_OPENMLS_STORAGE_RECORD_BYTES
            || !keys.insert(key.as_slice())
        {
            return Err(KeyStoreError::InvalidValue);
        }
        total = total
            .checked_add(key.len())
            .and_then(|total| total.checked_add(value.len()))
            .ok_or(KeyStoreError::LimitExceeded)?;
        if total > MAX_STORE_VALUE_BYTES {
            return Err(KeyStoreError::LimitExceeded);
        }
    }
    Ok(())
}

fn zeroize_provider_records(records: &mut [ProviderRecord]) {
    for (key, value) in records {
        key.zeroize();
        value.zeroize();
    }
}

impl PersistedConversation {
    fn from_metadata(metadata: ConversationPersistenceMetadata) -> Self {
        Self {
            group_id: metadata.group_id.to_string(),
            epoch: metadata.epoch,
            own_device_id: metadata.own_device_id.to_string(),
            audience: match metadata.audience {
                ConversationAudience::DirectMessage => PersistedAudience::DirectMessage,
                ConversationAudience::GroupDm => PersistedAudience::GroupDm,
            },
            pinned_roots: metadata
                .pinned_roots
                .into_iter()
                .map(|(user_id, root_key_pub)| PersistedRootPin {
                    user_id: user_id.to_string(),
                    root_key_pub: root_key_pub.to_vec(),
                })
                .collect(),
            delivery_service_signature_key: metadata
                .delivery_service
                .map(|identity| identity.signature_key().to_vec()),
            outbound_generation: metadata.outbound_generation,
            inbound: metadata
                .inbound
                .into_iter()
                .map(PersistedInboundQueue::from_metadata)
                .collect(),
            active: metadata.active,
        }
    }

    fn to_metadata(&self) -> Result<ConversationPersistenceMetadata, KeyStoreError> {
        let group_id =
            GroupId::try_from(self.group_id.clone()).map_err(|_| KeyStoreError::InvalidValue)?;
        let own_device_id = DeviceId::try_from(self.own_device_id.clone())
            .map_err(|_| KeyStoreError::InvalidValue)?;
        let pinned_roots = self
            .pinned_roots
            .iter()
            .map(|pin| {
                Ok((
                    UserId::try_from(pin.user_id.clone())
                        .map_err(|_| KeyStoreError::InvalidValue)?,
                    pin.root_key_pub
                        .as_slice()
                        .try_into()
                        .map_err(|_| KeyStoreError::InvalidValue)?,
                ))
            })
            .collect::<Result<Vec<_>, KeyStoreError>>()?;
        let inbound = self
            .inbound
            .iter()
            .map(PersistedInboundQueue::to_metadata)
            .collect::<Result<Vec<_>, _>>()?;
        let delivery_service = self
            .delivery_service_signature_key
            .as_deref()
            .map(|key| {
                let signature_key: [u8; 32] =
                    key.try_into().map_err(|_| KeyStoreError::InvalidValue)?;
                DeliveryServiceIdentity::try_new(signature_key)
                    .map_err(|_| KeyStoreError::InvalidValue)
            })
            .transpose()?;
        Ok(ConversationPersistenceMetadata {
            group_id,
            epoch: self.epoch,
            own_device_id,
            audience: match self.audience {
                PersistedAudience::DirectMessage => ConversationAudience::DirectMessage,
                PersistedAudience::GroupDm => ConversationAudience::GroupDm,
            },
            pinned_roots,
            delivery_service,
            outbound_generation: self.outbound_generation,
            inbound,
            active: self.active,
        })
    }
}

impl PersistedInboundQueue {
    fn from_metadata(metadata: InboundPersistenceMetadata) -> Self {
        Self {
            device_id: metadata.device_id.to_string(),
            next_generation: metadata.next_generation,
            pending: metadata
                .pending
                .into_iter()
                .map(PersistedMessage::from_message)
                .collect(),
        }
    }

    fn to_metadata(&self) -> Result<InboundPersistenceMetadata, KeyStoreError> {
        if self.pending.len() > usize::try_from(MAX_BUFFERED_GENERATION_GAP).unwrap_or(usize::MAX) {
            return Err(KeyStoreError::InvalidValue);
        }
        Ok(InboundPersistenceMetadata {
            device_id: DeviceId::try_from(self.device_id.clone())
                .map_err(|_| KeyStoreError::InvalidValue)?,
            next_generation: self.next_generation,
            pending: self
                .pending
                .iter()
                .map(PersistedMessage::to_message)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl PersistedMessage {
    fn from_message(message: DecryptedApplicationMessage) -> Self {
        Self {
            sender_user_id: message.sender_user_id.to_string(),
            sender_device_id: message.sender_device_id.to_string(),
            generation: message.generation,
            plaintext: message.plaintext,
        }
    }

    fn to_message(&self) -> Result<DecryptedApplicationMessage, KeyStoreError> {
        if self.plaintext.is_empty() || self.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES {
            return Err(KeyStoreError::InvalidValue);
        }
        Ok(DecryptedApplicationMessage {
            sender_user_id: UserId::try_from(self.sender_user_id.clone())
                .map_err(|_| KeyStoreError::InvalidValue)?,
            sender_device_id: DeviceId::try_from(self.sender_device_id.clone())
                .map_err(|_| KeyStoreError::InvalidValue)?,
            generation: self.generation,
            plaintext: self.plaintext.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{DeviceId, GroupId, UserId};

    use super::*;
    use crate::{
        generate_key_package_batch, generate_last_resort_key_package, load_root_identity,
        InMemoryKeyStore, PinnedUserIdentity, RootIdentityKey, DEFAULT_BATCH_SIZE,
    };

    #[test]
    fn initial_device_bootstrap_is_atomic_idempotent_and_retryable() {
        let root = RootIdentityKey::generate();
        let device = MlsDevice::generate(UserId::new(), DeviceId::new(), &root).unwrap();
        let mut packages = generate_key_package_batch(&device, 2).unwrap();
        packages.push(generate_last_resort_key_package(&device).unwrap());
        let store = InMemoryKeyStore::new();

        persist_initial_device_bootstrap(&store, &root, &device, &packages).unwrap();
        persist_initial_device_bootstrap(&store, &root, &device, &packages).unwrap();

        let restored = load_mls_client_state(&store).unwrap();
        assert_eq!(restored.device.user_id(), device.user_id());
        assert_eq!(restored.device.device_id(), device.device_id());
        assert!(restored.conversations.is_empty());
        let pending = load_pending_keypackage_upload(&store).unwrap();
        assert_eq!(pending.packages.len(), 3);
        assert_eq!(
            pending
                .packages
                .iter()
                .filter(|package| package.is_last_resort)
                .count(),
            1
        );
        assert_eq!(pending.packages[0].blob, packages[0].blob);
        assert_eq!(load_root_identity_rotation_sequence(&store), Ok(0));

        clear_pending_keypackage_upload(&store).unwrap();
        clear_pending_keypackage_upload(&store).unwrap();
        assert_eq!(
            load_pending_keypackage_upload(&store),
            Err(KeyStoreError::NotFound)
        );
    }

    #[test]
    fn initial_device_bootstrap_rejects_identity_conflict_and_corrupt_outbox() {
        let root = RootIdentityKey::generate();
        let other_root = RootIdentityKey::generate();
        let device = MlsDevice::generate(UserId::new(), DeviceId::new(), &root).unwrap();
        let mut packages = generate_key_package_batch(&device, 1).unwrap();
        packages.push(generate_last_resort_key_package(&device).unwrap());
        let store = InMemoryKeyStore::new();

        assert_eq!(
            persist_initial_device_bootstrap(&store, &other_root, &device, &packages),
            Err(KeyStoreError::InvalidValue)
        );
        assert!(store.list_keys().unwrap().is_empty());

        let other_device = MlsDevice::generate(device.user_id(), DeviceId::new(), &root).unwrap();
        let mut other_packages = generate_key_package_batch(&other_device, 1).unwrap();
        other_packages.push(generate_last_resort_key_package(&other_device).unwrap());
        assert_eq!(
            persist_initial_device_bootstrap(&store, &root, &device, &other_packages),
            Err(KeyStoreError::InvalidValue)
        );
        let duplicate_packages = vec![packages[1].clone(), packages[1].clone()];
        assert_eq!(
            persist_initial_device_bootstrap(&store, &root, &device, &duplicate_packages),
            Err(KeyStoreError::InvalidValue)
        );
        assert_eq!(
            persist_initial_device_bootstrap(&store, &root, &device, &packages[..1]),
            Err(KeyStoreError::InvalidValue)
        );
        assert!(store.list_keys().unwrap().is_empty());

        persist_initial_device_bootstrap(&store, &root, &device, &packages).unwrap();
        store
            .store(
                StoreKey::pending_keypackage_upload(),
                br#"{"version":1,"packages":[]}"#.to_vec(),
            )
            .unwrap();
        assert_eq!(
            load_pending_keypackage_upload(&store),
            Err(KeyStoreError::InvalidValue)
        );
    }

    #[test]
    fn pending_root_rotation_survives_restart_and_adopts_atomically() {
        let user_id = UserId::new();
        let device_id = DeviceId::new();
        let root = RootIdentityKey::generate();
        let device = MlsDevice::generate(user_id, device_id, &root).unwrap();
        let mut packages = generate_key_package_batch(&device, 1).unwrap();
        packages.push(generate_last_resort_key_package(&device).unwrap());
        let store = InMemoryKeyStore::new();
        persist_initial_device_bootstrap(&store, &root, &device, &packages).unwrap();

        let pending =
            prepare_pending_root_identity_rotation(&store, user_id, device_id, 0, &root).unwrap();
        let request = pending.wire_request().clone();
        assert!(store
            .exists(&StoreKey::pending_root_identity_rotation())
            .unwrap());
        assert_eq!(
            prepare_pending_root_identity_rotation(&store, user_id, device_id, 0, &root)
                .unwrap_err(),
            KeyStoreError::InvalidValue
        );
        drop(pending);

        let pending = load_pending_root_identity_rotation(&store).unwrap();
        assert_eq!(pending.wire_request(), &request);
        let response = RotateRootIdentityResponse {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            rotation_sequence: 1,
            previous_root_key_pub: root.public_key_bytes().to_vec(),
            new_root_key_pub: request.new_root_key_pub.clone(),
            revoked_device_count: 0,
            deleted_keypackage_count: 2,
            rotated_at_unix: 500,
        };
        pending.finish(&response, &store).unwrap();

        assert!(!store
            .exists(&StoreKey::pending_root_identity_rotation())
            .unwrap());
        assert_eq!(load_root_identity_rotation_sequence(&store), Ok(1));
        assert_eq!(
            load_root_identity(&store, &StoreKey::root_identity())
                .unwrap()
                .public_key_bytes()
                .to_vec(),
            request.new_root_key_pub
        );
        let state = load_mls_client_state(&store).unwrap();
        assert_eq!(state.device.user_id(), user_id);
        assert_eq!(state.device.device_id(), device_id);
        assert!(state.conversations.is_empty());
        assert_eq!(
            load_pending_keypackage_upload(&store)
                .unwrap()
                .packages
                .len(),
            DEFAULT_BATCH_SIZE + 1
        );
    }

    #[test]
    fn pending_root_rotation_rejects_tampering_and_mismatched_confirmation() {
        let user_id = UserId::new();
        let device_id = DeviceId::new();
        let root = RootIdentityKey::generate();
        let device = MlsDevice::generate(user_id, device_id, &root).unwrap();
        let mut packages = generate_key_package_batch(&device, 1).unwrap();
        packages.push(generate_last_resort_key_package(&device).unwrap());
        let store = InMemoryKeyStore::new();
        persist_initial_device_bootstrap(&store, &root, &device, &packages).unwrap();
        let pending =
            prepare_pending_root_identity_rotation(&store, user_id, device_id, 0, &root).unwrap();
        let request = pending.wire_request().clone();
        let mut response = RotateRootIdentityResponse {
            protocol_version: ROOT_IDENTITY_ROTATION_PROTOCOL_VERSION,
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            rotation_sequence: 1,
            previous_root_key_pub: root.public_key_bytes().to_vec(),
            new_root_key_pub: request.new_root_key_pub.clone(),
            revoked_device_count: 0,
            deleted_keypackage_count: 0,
            rotated_at_unix: 500,
        };
        response.new_root_key_pub[0] ^= 1;
        assert_eq!(
            pending.finish(&response, &store).unwrap_err(),
            KeyStoreError::InvalidValue
        );
        assert_eq!(
            load_root_identity(&store, &StoreKey::root_identity())
                .unwrap()
                .public_key_bytes(),
            root.public_key_bytes()
        );

        let mut encoded = store
            .load(&StoreKey::pending_root_identity_rotation())
            .unwrap()
            .to_vec();
        let last = encoded.last_mut().unwrap();
        *last ^= 1;
        store
            .store(StoreKey::pending_root_identity_rotation(), encoded)
            .unwrap();
        assert_eq!(
            load_pending_root_identity_rotation(&store).unwrap_err(),
            KeyStoreError::InvalidValue
        );
    }

    fn joined_conversations() -> (MlsDevice, MlsConversation, MlsDevice, MlsConversation) {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_keypackage = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, pending) = MlsConversation::create_two_member(
            group_id,
            &alice,
            PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public()),
            &bob_keypackage,
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public()),
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        (alice, alice_group, bob, bob_group)
    }

    #[test]
    fn restart_preserves_mls_ratchets_and_generation_gap_buffer() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let first = alice_group
            .encrypt_application_message(&alice, b"first across restart")
            .unwrap();
        let second = alice_group
            .encrypt_application_message(&alice, b"second across restart")
            .unwrap();
        let gap = bob_group
            .decrypt_application_message(&bob, &second)
            .unwrap();
        assert!(gap.ready_messages.is_empty());
        assert!(gap.messages_may_be_missing);

        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        drop(bob_group);
        drop(bob);

        let mut restored = load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations.len(), 1);
        let outcome = restored.conversations[0]
            .decrypt_application_message(&restored.device, &first)
            .unwrap();
        assert_eq!(
            outcome
                .ready_messages
                .iter()
                .map(|message| message.plaintext.as_slice())
                .collect::<Vec<_>>(),
            vec![
                b"first across restart".as_slice(),
                b"second across restart".as_slice()
            ]
        );
        assert!(!outcome.messages_may_be_missing);
    }

    #[test]
    fn restart_preserves_commit_epoch_and_signer() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let pending = bob_group.create_self_update(&bob).unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();
        alice_group
            .process_incoming_commit(
                &alice,
                &crate::EncryptedGroupCommit {
                    group_id: pending.group_id,
                    prior_epoch: pending.prior_epoch,
                    epoch: pending.epoch,
                    committer_device_id: pending.committer_device_id,
                    commit_blob: pending.commit_blob,
                },
            )
            .unwrap();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();

        let mut restored = load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), 2);
        let encrypted = restored.conversations[0]
            .encrypt_application_message(&restored.device, b"signed after restart")
            .unwrap();
        assert_eq!(
            alice_group
                .decrypt_application_message(&alice, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"signed after restart"
        );
    }

    #[test]
    fn restart_preserves_pending_intent_for_epoch_conflict_rebase() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let rejected = alice_group.create_self_update(&alice).unwrap();
        let winner = bob_group.create_self_update(&bob).unwrap();
        assert_eq!(rejected.epoch, winner.epoch);
        bob_group.accept_pending_commit(&bob).unwrap();
        let accepted = crate::EncryptedGroupCommit {
            group_id: winner.group_id,
            prior_epoch: winner.prior_epoch,
            epoch: winner.epoch,
            committer_device_id: winner.committer_device_id,
            commit_blob: winner.commit_blob,
        };

        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[&alice_group]).unwrap();
        drop(alice_group);
        drop(alice);

        let mut restored = load_mls_client_state(&store).unwrap();
        let crate::PendingCommitRebase::Rebased(rebased) = restored.conversations[0]
            .rebase_pending_commit(&restored.device, &accepted)
            .unwrap()
        else {
            panic!("durably restored intent must be rebased");
        };
        assert_eq!(rebased.prior_epoch, 2);
        assert_eq!(rebased.epoch, 3);
        restored.conversations[0]
            .accept_pending_commit(&restored.device)
            .unwrap();
        bob_group
            .process_incoming_commit(
                &bob,
                &crate::EncryptedGroupCommit {
                    group_id: rebased.group_id,
                    prior_epoch: rebased.prior_epoch,
                    epoch: rebased.epoch,
                    committer_device_id: rebased.committer_device_id,
                    commit_blob: rebased.commit_blob,
                },
            )
            .unwrap();
        let encrypted = restored.conversations[0]
            .encrypt_application_message(&restored.device, b"rebased after restart")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"rebased after restart"
        );
    }

    #[test]
    fn restart_preserves_multi_device_membership_and_ratchets() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_second = MlsDevice::generate(bob.user_id(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, initial) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let second_package = generate_key_package_batch(&bob_second, 1)
            .unwrap()
            .remove(0)
            .blob;
        let add = alice_group
            .create_add_device(&alice, bob_pin, &second_package)
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        bob_group
            .process_incoming_commit(
                &bob,
                &crate::EncryptedGroupCommit {
                    group_id,
                    prior_epoch: add.prior_epoch,
                    epoch: add.epoch,
                    committer_device_id: add.committer_device_id,
                    commit_blob: add.commit_blob.clone(),
                },
            )
            .unwrap();
        let bob_second_group = MlsConversation::join_from_welcome(
            group_id,
            &bob_second,
            alice_pin,
            add.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob_second, &[&bob_second_group]).unwrap();
        drop(bob_second_group);
        drop(bob_second);
        let mut restored = load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), 2);

        let from_alice = alice_group
            .encrypt_application_message(&alice, b"after multi-device restart")
            .unwrap();
        assert_eq!(
            restored.conversations[0]
                .decrypt_application_message(&restored.device, &from_alice)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"after multi-device restart"
        );
        let from_restored = restored.conversations[0]
            .encrypt_application_message(&restored.device, b"restored device reply")
            .unwrap();
        assert_eq!(
            alice_group
                .decrypt_application_message(&alice, &from_restored)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"restored device reply"
        );

        let removal = alice_group
            .create_remove_device(&alice, restored.device.device_id())
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        restored.conversations[0]
            .process_incoming_commit(
                &restored.device,
                &crate::EncryptedGroupCommit {
                    group_id,
                    prior_epoch: removal.prior_epoch,
                    epoch: removal.epoch,
                    committer_device_id: removal.committer_device_id,
                    commit_blob: removal.commit_blob,
                },
            )
            .unwrap();
        persist_mls_client_state(&store, &restored.device, &[&restored.conversations[0]]).unwrap();
        let mut removed = load_mls_client_state(&store).unwrap();
        assert_eq!(removed.conversations[0].epoch(), 3);
        assert_eq!(
            removed.conversations[0]
                .encrypt_application_message(&removed.device, b"must remain evicted")
                .unwrap_err(),
            crate::ConversationError::NotActive
        );
    }

    #[test]
    fn restart_preserves_pending_commit_acceptance_boundary() {
        let bob_root = RootIdentityKey::generate();
        let alice_root = RootIdentityKey::generate();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let alice_keypackage = generate_key_package_batch(&alice, 1)
            .unwrap()
            .remove(0)
            .blob;
        let (bob_group, _) = MlsConversation::create_two_member(
            GroupId::new(),
            &bob,
            PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public()),
            &alice_keypackage,
        )
        .unwrap();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();

        let mut restored = load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), 0);
        restored.conversations[0]
            .accept_pending_commit(&restored.device)
            .unwrap();
        assert_eq!(restored.conversations[0].epoch(), 1);
        assert!(restored.conversations[0]
            .encrypt_application_message(&restored.device, b"accepted after restart")
            .is_ok());
    }

    #[test]
    fn corrupt_or_unknown_checkpoint_fails_closed() {
        let store = InMemoryKeyStore::new();
        store
            .store(StoreKey::mls_client_state(), br#"{"version":99}"#.to_vec())
            .unwrap();
        assert_eq!(
            load_mls_client_state(&store).unwrap_err(),
            KeyStoreError::InvalidValue
        );

        store
            .store(
                StoreKey::mls_client_state(),
                vec![b'x'; MAX_STORE_VALUE_BYTES],
            )
            .unwrap();
        assert_eq!(
            load_mls_client_state(&store).unwrap_err(),
            KeyStoreError::InvalidValue
        );
    }

    #[test]
    fn version_one_checkpoint_migrates_to_direct_message_audience() {
        let (_, _, bob, bob_group) = joined_conversations();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let bytes = store.load(&StoreKey::mls_client_state()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["version"] = serde_json::Value::from(LEGACY_MLS_CLIENT_STATE_VERSIONS[0]);
        value["conversations"][0]
            .as_object_mut()
            .unwrap()
            .remove("audience");
        store
            .store(
                StoreKey::mls_client_state(),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();

        let restored = load_mls_client_state(&store).unwrap();
        assert_eq!(
            restored.conversations[0].audience(),
            ConversationAudience::DirectMessage
        );

        value["conversations"][0]["audience"] = serde_json::Value::from("group_dm");
        store
            .store(
                StoreKey::mls_client_state(),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();
        assert_eq!(
            load_mls_client_state(&store).unwrap_err(),
            KeyStoreError::InvalidValue
        );
    }

    #[test]
    fn mixed_epoch_checkpoint_fails_closed() {
        let (_, _, bob, bob_group) = joined_conversations();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let bytes = store.load(&StoreKey::mls_client_state()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["conversations"][0]["epoch"] = serde_json::Value::from(99_u64);
        store
            .store(
                StoreKey::mls_client_state(),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();

        assert_eq!(
            load_mls_client_state(&store).unwrap_err(),
            KeyStoreError::InvalidValue
        );
    }
}
