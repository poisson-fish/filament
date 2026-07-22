//! Versioned, encrypted checkpoints for native MLS client state.
//!
//! OpenMLS persists group secrets into its provider as operations complete.
//! This module snapshots that complete provider together with Filament's
//! identity pins and generation queues into one [`LocalKeyStore`] record. A
//! mailbox acknowledgment is safe only after this checkpoint and any released
//! message history have both been durably written by the native runtime.

use std::collections::HashSet;

use filament_core::{DeviceCertificate, DeviceId, GroupId, UserId};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    conversation::{ConversationPersistenceMetadata, InboundPersistenceMetadata},
    keypackage::ProviderRecord,
    ConversationAudience, DecryptedApplicationMessage, ExternalCommitRecoveryInfo, KeyStoreError,
    LocalKeyStore, MlsConversation, MlsDevice, PendingGroupCommit, PinnedUserIdentity, StoreKey,
    MAX_APPLICATION_PLAINTEXT_BYTES, MAX_BUFFERED_GENERATION_GAP, MAX_STORE_VALUE_BYTES,
};

const MLS_CLIENT_STATE_VERSION: u16 = 2;
const LEGACY_MLS_CLIENT_STATE_VERSION: u16 = 1;
const MAX_MLS_CONVERSATIONS: usize = 1_024;
const MAX_OPENMLS_STORAGE_RECORDS: usize = 16_384;
const MAX_OPENMLS_STORAGE_KEY_BYTES: usize = 4_096;
const MAX_OPENMLS_STORAGE_RECORD_BYTES: usize = 1024 * 1024;

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
        let peer_pin_matches = metadata
            .pinned_roots
            .iter()
            .any(|(user_id, root_key)| *user_id == peer.user_id && *root_key == peer.root_key_pub);
        if !peer_pin_matches
            || (metadata.active && recovery.epoch <= metadata.epoch)
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
            current.recover_by_external_commit(recovery, &candidate.device, peer)?;
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
    if ![LEGACY_MLS_CLIENT_STATE_VERSION, MLS_CLIENT_STATE_VERSION].contains(&snapshot.version)
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
        if snapshot.version == LEGACY_MLS_CLIENT_STATE_VERSION
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

fn clone_client_state(encoded: &[u8]) -> Result<MlsClientState, KeyStoreError> {
    let mut snapshot: PersistedClientState =
        serde_json::from_slice(encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    let cloned = restore_snapshot(&snapshot);
    snapshot.zeroize();
    cloned
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
        Ok(ConversationPersistenceMetadata {
            group_id,
            epoch: self.epoch,
            own_device_id,
            audience: match self.audience {
                PersistedAudience::DirectMessage => ConversationAudience::DirectMessage,
                PersistedAudience::GroupDm => ConversationAudience::GroupDm,
            },
            pinned_roots,
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
        generate_key_package_batch, InMemoryKeyStore, PinnedUserIdentity, RootIdentityKey,
    };

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
        value["version"] = serde_json::Value::from(LEGACY_MLS_CLIENT_STATE_VERSION);
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
