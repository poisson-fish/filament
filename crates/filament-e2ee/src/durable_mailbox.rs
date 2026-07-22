//! Durable native coordination for MLS mailboxes.
//!
//! Mailbox processing consumes forward-secure MLS state. This module binds the
//! resulting checkpoint, authenticated plaintext history, and acknowledgment
//! outbox into one encrypted-store transaction. Network acknowledgments are
//! exposed only after that transaction commits and survive process restarts.

use std::collections::HashSet;

use filament_core::{DeviceId, GroupId, UserId};
use filament_protocol::{
    AckE2eeCommitsRequest, AckE2eeMessagesRequest, E2eeCommitMailboxResponse, E2eeMailboxResponse,
    PostCommitResponse, MAX_E2EE_COMMIT_ACK_BATCH_SIZE, MAX_E2EE_MESSAGE_ACK_BATCH_SIZE,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    persistence::encode_mls_client_state, process_commit_mailbox, process_message_mailbox,
    ConversationError, DecryptedApplicationMessage, ExternalCommitRecoveryInfo, KeyStoreError,
    LocalKeyStore, MlsClientState, MlsConversation, PendingExternalCommitRecovery,
    PinnedUserIdentity, RejectedMailboxCommit, RejectedMailboxMessage, StoreKey,
    MAX_APPLICATION_PLAINTEXT_BYTES, MAX_STORE_VALUE_BYTES,
};

const HISTORY_RECORD_VERSION: u16 = 1;
const MAX_LOCAL_HISTORY_RECORD_BYTES: usize = (MAX_APPLICATION_PLAINTEXT_BYTES * 4) + 2_048;
const MAX_UNIX_TIMESTAMP: i64 = 253_402_300_799;

/// A native mailbox coordinator that fails shut after any uncertain write.
pub struct DurableMlsClient {
    state: Option<MlsClientState>,
}

impl core::fmt::Debug for DurableMlsClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DurableMlsClient")
            .field("ready", &self.state.is_some())
            .field(
                "conversation_count",
                &self
                    .state
                    .as_ref()
                    .map_or(0, |state| state.conversations.len()),
            )
            .field("state", &"<MLS key material omitted>")
            .finish()
    }
}

/// Errors at the native durability and mailbox boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DurableMailboxError {
    /// The runtime was shut down after an uncertain persistence failure.
    #[error("durable MLS runtime is unavailable until reloaded")]
    Unavailable,
    /// A previous durable acknowledgment must be submitted first.
    #[error("a durable mailbox acknowledgment is already pending")]
    PendingAcknowledgment,
    /// The requested group has no local MLS state.
    #[error("MLS conversation is not available locally")]
    ConversationNotFound,
    /// Untrusted mailbox data failed validation or MLS authentication.
    #[error(transparent)]
    Conversation(#[from] ConversationError),
    /// Encrypted local persistence failed.
    #[error(transparent)]
    KeyStore(#[from] KeyStoreError),
}

/// A message mailbox result whose acknowledgment is already durable.
pub struct DurableMessageMailboxBatch {
    /// Authenticated messages newly released in generation order.
    pub ready_messages: Vec<DecryptedApplicationMessage>,
    /// Entries rejected without acknowledgment.
    pub rejected_messages: Vec<RejectedMailboxMessage>,
    /// Whether later authenticated generations remain behind a gap.
    pub messages_may_be_missing: bool,
    /// Durable request safe for the native network boundary to submit.
    pub acknowledgment: Option<AckE2eeMessagesRequest>,
}

/// A commit mailbox result whose successful prefix and acknowledgment are durable.
pub struct DurableCommitMailboxBatch {
    /// Epochs newly established and checkpointed by this call.
    pub processed_epochs: Vec<u64>,
    /// First rejected epoch, if any.
    pub rejected_commit: Option<RejectedMailboxCommit>,
    /// Durable request safe for the native network boundary to submit.
    pub acknowledgment: Option<AckE2eeCommitsRequest>,
}

/// One authenticated plaintext record loaded from encrypted native history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMailboxMessage {
    pub message_id: String,
    pub group_id: GroupId,
    pub created_at_unix: i64,
    pub message: DecryptedApplicationMessage,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryRecordRef<'a> {
    version: u16,
    message_id: &'a str,
    group_id: String,
    created_at_unix: i64,
    sender_user_id: String,
    sender_device_id: String,
    generation: u64,
    plaintext: &'a [u8],
}

#[derive(Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct HistoryRecord {
    version: u16,
    message_id: String,
    group_id: String,
    created_at_unix: i64,
    sender_user_id: String,
    sender_device_id: String,
    generation: u64,
    plaintext: Vec<u8>,
}

impl DurableMlsClient {
    /// Restore a fully validated native runtime from encrypted storage.
    ///
    /// # Errors
    /// Returns a local-store error for missing, corrupt, or unavailable state.
    pub fn load(store: &dyn LocalKeyStore) -> Result<Self, KeyStoreError> {
        Ok(Self {
            state: Some(crate::load_mls_client_state(store)?),
        })
    }

    /// Adopt already-validated state. Callers must persist it before accepting
    /// network mailboxes.
    #[must_use]
    pub const fn from_state(state: MlsClientState) -> Self {
        Self { state: Some(state) }
    }

    /// Whether the runtime may currently perform cryptographic operations.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.state.is_some()
    }

    /// Reload the last complete checkpoint after an uncertain local write.
    ///
    /// # Errors
    /// Leaves the runtime unavailable if the checkpoint cannot be restored.
    pub fn reload(&mut self, store: &dyn LocalKeyStore) -> Result<(), KeyStoreError> {
        self.state = None;
        self.state = Some(crate::load_mls_client_state(store)?);
        Ok(())
    }

    /// Prepare an isolated external-commit recovery candidate.
    ///
    /// Existing mailbox acknowledgment outboxes for the group must be drained
    /// first. The live state remains usable and unchanged until
    /// [`Self::confirm_external_commit_recovery`] receives an exact accepted
    /// epoch and durably checkpoints the candidate.
    ///
    /// # Errors
    /// Returns a durability or fail-closed MLS validation error.
    pub fn prepare_external_commit_recovery(
        &self,
        store: &dyn LocalKeyStore,
        peer: PinnedUserIdentity,
        recovery: &ExternalCommitRecoveryInfo,
    ) -> Result<PendingExternalCommitRecovery, DurableMailboxError> {
        if store.exists(&message_ack_key(recovery.group_id)?)?
            || store.exists(&commit_ack_key(recovery.group_id)?)?
        {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        state
            .prepare_external_commit_recovery(peer, recovery)
            .map_err(Into::into)
    }

    /// Adopt an externally recovered group after exact server acceptance.
    ///
    /// The replacement provider and all conversations are checkpointed as one
    /// encrypted record before they become usable. An uncertain local write
    /// shuts the runtime down until [`Self::reload`] restores the last complete
    /// checkpoint.
    ///
    /// # Errors
    /// Rejects stale candidates and untrusted acceptance metadata without
    /// changing live state. Persistence failure makes the runtime unavailable.
    pub fn confirm_external_commit_recovery(
        &mut self,
        store: &dyn LocalKeyStore,
        recovery: PendingExternalCommitRecovery,
        response: &PostCommitResponse,
    ) -> Result<(), DurableMailboxError> {
        let pending = recovery.pending_commit();
        if !response.accepted || response.epoch != pending.epoch {
            return Err(ConversationError::MetadataMismatch.into());
        }
        let group_id = pending.group_id;
        let current = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        if !current
            .conversations
            .iter()
            .any(|conversation| conversation.group_id() == group_id)
        {
            return Err(DurableMailboxError::ConversationNotFound);
        }
        let current_conversations = current.conversations.iter().collect::<Vec<_>>();
        let current_checkpoint = encode_mls_client_state(&current.device, &current_conversations)?;
        if current_checkpoint.as_slice() != recovery.base_checkpoint() {
            return Err(ConversationError::MetadataMismatch.into());
        }

        let candidate = recovery.into_state();
        self.state = None;
        let conversations = candidate.conversations.iter().collect::<Vec<_>>();
        if let Err(error) =
            crate::persist_mls_client_state(store, &candidate.device, &conversations)
        {
            return Err(error.into());
        }
        self.state = Some(candidate);
        Ok(())
    }

    /// Process one bounded message page and atomically persist its effects.
    ///
    /// A previous pending acknowledgment must be confirmed first. On any batch
    /// write failure the in-memory state is discarded; callers must reload the
    /// last complete checkpoint before continuing.
    ///
    /// # Errors
    /// Returns fail-closed mailbox, conversation, or durability errors.
    pub fn process_message_mailbox(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        page: E2eeMailboxResponse,
    ) -> Result<DurableMessageMailboxBatch, DurableMailboxError> {
        if store.exists(&message_ack_key(group_id)?)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let Some(position) = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
        else {
            self.state = Some(state);
            return Err(DurableMailboxError::ConversationNotFound);
        };
        let batch = match process_message_mailbox(
            &mut state.conversations[position],
            &state.device,
            page,
        ) {
            Ok(batch) => batch,
            Err(error) => {
                self.state = Some(state);
                return Err(error.into());
            }
        };

        if let Some(acknowledgment) = &batch.pending_acknowledgment {
            let entries = message_durability_entries(&state, group_id, &batch, acknowledgment)?;
            if let Err(error) = store.store_batch(entries) {
                // Consumed forward-secure state must not remain usable when its
                // durability is uncertain. Reload restores the previous atomic state.
                return Err(error.into());
            }
        }

        self.state = Some(state);
        Ok(DurableMessageMailboxBatch {
            ready_messages: batch.ready_messages,
            rejected_messages: batch.rejected_messages,
            messages_may_be_missing: batch.messages_may_be_missing,
            acknowledgment: batch.pending_acknowledgment,
        })
    }

    /// Process an ordered commit page and atomically persist its success prefix.
    ///
    /// # Errors
    /// Returns fail-closed mailbox, conversation, or durability errors.
    pub fn process_commit_mailbox(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        peer: PinnedUserIdentity,
        page: E2eeCommitMailboxResponse,
    ) -> Result<DurableCommitMailboxBatch, DurableMailboxError> {
        if store.exists(&commit_ack_key(group_id)?)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let existing_position = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id);
        let mut conversation =
            existing_position.map(|position| state.conversations.remove(position));
        let batch =
            match process_commit_mailbox(&mut conversation, &state.device, group_id, peer, page) {
                Ok(batch) => batch,
                Err(error) => {
                    if let Some(conversation) = conversation {
                        insert_conversation(
                            &mut state.conversations,
                            existing_position,
                            conversation,
                        );
                    }
                    self.state = Some(state);
                    return Err(error.into());
                }
            };
        if let Some(conversation) = conversation {
            insert_conversation(&mut state.conversations, existing_position, conversation);
        }

        if let Some(acknowledgment) = &batch.pending_acknowledgment {
            let checkpoint = encode_state(&state)?;
            let outbox = encode_json(acknowledgment)?;
            if let Err(error) = store.store_batch(vec![
                (StoreKey::mls_client_state(), checkpoint),
                (commit_ack_key(group_id)?, outbox),
            ]) {
                return Err(error.into());
            }
        }

        self.state = Some(state);
        Ok(DurableCommitMailboxBatch {
            processed_epochs: batch.processed_epochs,
            rejected_commit: batch.rejected_commit,
            acknowledgment: batch.pending_acknowledgment,
        })
    }
}

/// Load a restart-safe message acknowledgment from the native outbox.
///
/// # Errors
/// Rejects corrupt, cross-device, duplicate, oversized, or non-canonical data.
pub fn pending_message_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    expected_device_id: DeviceId,
) -> Result<Option<AckE2eeMessagesRequest>, KeyStoreError> {
    let key = message_ack_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let acknowledgment: AckE2eeMessagesRequest =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    validate_message_ack(&acknowledgment, expected_device_id)?;
    Ok(Some(acknowledgment))
}

/// Remove a message acknowledgment only after a successful server response.
///
/// # Errors
/// Rejects a request that does not exactly match the durable outbox record.
pub fn confirm_message_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    submitted: &AckE2eeMessagesRequest,
) -> Result<(), KeyStoreError> {
    confirm_ack(store, &message_ack_key(group_id)?, submitted)
}

/// Load a restart-safe commit acknowledgment from the native outbox.
///
/// # Errors
/// Rejects corrupt, cross-device, duplicate, oversized, or invalid epoch data.
pub fn pending_commit_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    expected_device_id: DeviceId,
) -> Result<Option<AckE2eeCommitsRequest>, KeyStoreError> {
    let key = commit_ack_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let acknowledgment: AckE2eeCommitsRequest =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    validate_commit_ack(&acknowledgment, expected_device_id)?;
    Ok(Some(acknowledgment))
}

/// Remove a commit acknowledgment only after a successful server response.
///
/// # Errors
/// Rejects a request that does not exactly match the durable outbox record.
pub fn confirm_commit_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    submitted: &AckE2eeCommitsRequest,
) -> Result<(), KeyStoreError> {
    confirm_ack(store, &commit_ack_key(group_id)?, submitted)
}

/// Read one authenticated message from encrypted native history.
///
/// # Errors
/// Rejects non-canonical IDs, corrupt records, mismatched keys, and all bounds
/// violations before plaintext is returned to the native presentation layer.
pub fn load_stored_message(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    message_id: &str,
) -> Result<StoredMailboxMessage, KeyStoreError> {
    validate_ulid(message_id)?;
    let encoded = store.load(&history_key(group_id, message_id)?)?;
    if encoded.len() > MAX_LOCAL_HISTORY_RECORD_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut record: HistoryRecord =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    if record.version != HISTORY_RECORD_VERSION
        || record.message_id != message_id
        || !(0..=MAX_UNIX_TIMESTAMP).contains(&record.created_at_unix)
        || record.plaintext.is_empty()
        || record.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
    {
        record.zeroize();
        return Err(KeyStoreError::InvalidValue);
    }
    let parsed_group = match GroupId::try_from(record.group_id.clone()) {
        Ok(parsed) if parsed == group_id => parsed,
        _ => {
            record.zeroize();
            return Err(KeyStoreError::InvalidValue);
        }
    };
    let Ok(sender_user_id) = UserId::try_from(record.sender_user_id.clone()) else {
        record.zeroize();
        return Err(KeyStoreError::InvalidValue);
    };
    let Ok(sender_device_id) = DeviceId::try_from(record.sender_device_id.clone()) else {
        record.zeroize();
        return Err(KeyStoreError::InvalidValue);
    };
    let stored = StoredMailboxMessage {
        message_id: core::mem::take(&mut record.message_id),
        group_id: parsed_group,
        created_at_unix: record.created_at_unix,
        message: DecryptedApplicationMessage {
            sender_user_id,
            sender_device_id,
            generation: record.generation,
            plaintext: core::mem::take(&mut record.plaintext),
        },
    };
    record.zeroize();
    Ok(stored)
}

fn message_durability_entries(
    state: &MlsClientState,
    group_id: GroupId,
    batch: &crate::MailboxDecryptionBatch,
    acknowledgment: &AckE2eeMessagesRequest,
) -> Result<Vec<(StoreKey, Vec<u8>)>, DurableMailboxError> {
    if batch.authenticated_messages.len() != acknowledgment.message_ids.len() {
        return Err(KeyStoreError::InvalidValue.into());
    }
    let mut entries = Vec::with_capacity(batch.authenticated_messages.len() + 2);
    entries.push((StoreKey::mls_client_state(), encode_state(state)?));
    for authenticated in &batch.authenticated_messages {
        let message = &authenticated.message;
        let record = HistoryRecordRef {
            version: HISTORY_RECORD_VERSION,
            message_id: &authenticated.message_id,
            group_id: group_id.to_string(),
            created_at_unix: authenticated.created_at_unix,
            sender_user_id: message.sender_user_id.to_string(),
            sender_device_id: message.sender_device_id.to_string(),
            generation: message.generation,
            plaintext: &message.plaintext,
        };
        let encoded = encode_json(&record)?;
        if encoded.len() > MAX_LOCAL_HISTORY_RECORD_BYTES {
            return Err(KeyStoreError::LimitExceeded.into());
        }
        entries.push((history_key(group_id, &authenticated.message_id)?, encoded));
    }
    entries.push((message_ack_key(group_id)?, encode_json(acknowledgment)?));
    Ok(entries)
}

fn encode_state(state: &MlsClientState) -> Result<Vec<u8>, KeyStoreError> {
    let conversations = state.conversations.iter().collect::<Vec<_>>();
    encode_mls_client_state(&state.device, &conversations).map(|encoded| encoded.to_vec())
}

fn encode_json(value: &impl Serialize) -> Result<Vec<u8>, KeyStoreError> {
    let encoded = serde_json::to_vec(value).map_err(|_| KeyStoreError::BackendError)?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    Ok(Zeroizing::new(encoded).to_vec())
}

fn insert_conversation(
    conversations: &mut Vec<MlsConversation>,
    previous_position: Option<usize>,
    conversation: MlsConversation,
) {
    if let Some(position) = previous_position {
        conversations.insert(position, conversation);
    } else {
        conversations.push(conversation);
    }
}

fn validate_message_ack(
    acknowledgment: &AckE2eeMessagesRequest,
    expected_device_id: DeviceId,
) -> Result<(), KeyStoreError> {
    if acknowledgment.device_id != expected_device_id.to_string()
        || acknowledgment.message_ids.is_empty()
        || acknowledgment.message_ids.len() > MAX_E2EE_MESSAGE_ACK_BATCH_SIZE
    {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut ids = HashSet::with_capacity(acknowledgment.message_ids.len());
    for id in &acknowledgment.message_ids {
        validate_ulid(id)?;
        if !ids.insert(id) {
            return Err(KeyStoreError::InvalidValue);
        }
    }
    Ok(())
}

fn validate_commit_ack(
    acknowledgment: &AckE2eeCommitsRequest,
    expected_device_id: DeviceId,
) -> Result<(), KeyStoreError> {
    if acknowledgment.device_id != expected_device_id.to_string()
        || acknowledgment.epochs.is_empty()
        || acknowledgment.epochs.len() > MAX_E2EE_COMMIT_ACK_BATCH_SIZE
        || acknowledgment.epochs.contains(&0)
        || !acknowledgment
            .epochs
            .windows(2)
            .all(|epochs| epochs[0] < epochs[1])
    {
        return Err(KeyStoreError::InvalidValue);
    }
    Ok(())
}

fn confirm_ack<T>(
    store: &dyn LocalKeyStore,
    key: &StoreKey,
    submitted: &T,
) -> Result<(), KeyStoreError>
where
    T: Serialize,
{
    let durable = store.load(key)?;
    let submitted = encode_json(submitted)?;
    if durable.as_slice() != submitted.as_slice() {
        return Err(KeyStoreError::InvalidValue);
    }
    store.remove(key)
}

fn history_key(group_id: GroupId, message_id: &str) -> Result<StoreKey, KeyStoreError> {
    validate_ulid(message_id)?;
    StoreKey::new(format!("history:{group_id}:{message_id}"))
}

fn message_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:message_ack:{group_id}"))
}

fn commit_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:commit_ack:{group_id}"))
}

fn validate_ulid(value: &str) -> Result<(), KeyStoreError> {
    if Ulid::from_string(value).is_ok_and(|parsed| parsed.to_string() == value) {
        Ok(())
    } else {
        Err(KeyStoreError::InvalidIdentifier)
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{ConversationCrypto, DeviceId, GroupId, UserId};
    use filament_protocol::{
        E2eeCommitMailboxEntry, E2eeMailboxMessage, GroupInfoResponse, PostCommitResponse,
    };

    use super::*;
    use crate::{
        generate_key_package_batch, persist_mls_client_state, InMemoryKeyStore, MlsDevice,
        RootIdentityKey,
    };

    struct JoinedFixture {
        alice: MlsDevice,
        alice_group: MlsConversation,
        bob: MlsDevice,
        bob_group: MlsConversation,
        group_id: GroupId,
    }

    fn joined_fixture() -> JoinedFixture {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, pending) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &key_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            alice_pin,
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        JoinedFixture {
            alice,
            alice_group,
            bob,
            bob_group,
            group_id,
        }
    }

    fn message_entry(
        message_id: String,
        encrypted: crate::EncryptedApplicationMessage,
    ) -> E2eeMailboxMessage {
        E2eeMailboxMessage {
            message_id,
            crypto: encrypted.crypto.as_str().to_owned(),
            epoch: encrypted.epoch,
            suite_id: encrypted.suite.as_u16(),
            sender_device_id: encrypted.sender_device_id.to_string(),
            message_blob: encrypted.message_blob,
            created_at_unix: 10,
            expires_at_unix: 20,
        }
    }

    #[test]
    fn message_history_state_and_ack_survive_restart_as_one_boundary() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
            ..
        } = joined_fixture();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let bob_device_id = bob.device_id();
        let message_id = Ulid::new().to_string();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"durable hello")
            .unwrap();
        let page = E2eeMailboxResponse {
            messages: vec![message_entry(message_id.clone(), encrypted)],
            next_after_message_id: Some(message_id.clone()),
        };

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let batch = runtime
            .process_message_mailbox(&store, group_id, page.clone())
            .unwrap();
        assert_eq!(batch.ready_messages[0].plaintext, b"durable hello");
        let acknowledgment = batch.acknowledgment.unwrap();
        assert_eq!(acknowledgment.message_ids, vec![message_id.clone()]);
        assert_eq!(
            load_stored_message(&store, group_id, &message_id)
                .unwrap()
                .message
                .plaintext,
            b"durable hello"
        );

        drop(runtime);
        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            pending_message_acknowledgment(&store, group_id, bob_device_id).unwrap(),
            Some(acknowledgment.clone())
        );
        assert!(matches!(
            restarted.process_message_mailbox(&store, group_id, page),
            Err(DurableMailboxError::PendingAcknowledgment)
        ));

        let wrong = AckE2eeMessagesRequest {
            device_id: acknowledgment.device_id.clone(),
            message_ids: vec![Ulid::new().to_string()],
        };
        assert_eq!(
            confirm_message_acknowledgment(&store, group_id, &wrong),
            Err(KeyStoreError::InvalidValue)
        );
        confirm_message_acknowledgment(&store, group_id, &acknowledgment).unwrap();
        assert_eq!(
            pending_message_acknowledgment(&store, group_id, bob_device_id).unwrap(),
            None
        );
    }

    #[test]
    fn downgrade_hint_is_surfaced_and_never_persisted_or_acked() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
            ..
        } = joined_fixture();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let message_id = Ulid::new().to_string();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"must stay encrypted")
            .unwrap();
        let mut entry = message_entry(message_id.clone(), encrypted);
        entry.crypto = ConversationCrypto::Plaintext.as_str().to_owned();
        let page = E2eeMailboxResponse {
            messages: vec![entry],
            next_after_message_id: Some(message_id.clone()),
        };

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let batch = runtime
            .process_message_mailbox(&store, group_id, page)
            .unwrap();
        assert!(batch.ready_messages.is_empty());
        assert!(batch.acknowledgment.is_none());
        assert_eq!(batch.rejected_messages.len(), 1);
        assert_eq!(
            batch.rejected_messages[0].error,
            ConversationError::CryptoModeMismatch
        );
        assert_eq!(
            load_stored_message(&store, group_id, &message_id),
            Err(KeyStoreError::NotFound)
        );
        assert!(runtime.is_ready());
    }

    struct RejectBatchStore {
        inner: InMemoryKeyStore,
    }

    impl LocalKeyStore for RejectBatchStore {
        fn store(&self, key: StoreKey, value: Vec<u8>) -> Result<(), KeyStoreError> {
            self.inner.store(key, value)
        }

        fn store_batch(&self, _entries: Vec<(StoreKey, Vec<u8>)>) -> Result<(), KeyStoreError> {
            Err(KeyStoreError::BackendError)
        }

        fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
            self.inner.load(key)
        }

        fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError> {
            self.inner.remove(key)
        }

        fn exists(&self, key: &StoreKey) -> Result<bool, KeyStoreError> {
            self.inner.exists(key)
        }

        fn list_keys(&self) -> Result<Vec<StoreKey>, KeyStoreError> {
            self.inner.list_keys()
        }
    }

    #[test]
    fn failed_atomic_write_shuts_runtime_without_history_or_ack() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
            ..
        } = joined_fixture();
        let store = RejectBatchStore {
            inner: InMemoryKeyStore::new(),
        };
        persist_mls_client_state(&store.inner, &bob, &[&bob_group]).unwrap();
        let device_id = bob.device_id();
        let message_id = Ulid::new().to_string();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"never partially durable")
            .unwrap();
        let page = E2eeMailboxResponse {
            messages: vec![message_entry(message_id.clone(), encrypted)],
            next_after_message_id: Some(message_id.clone()),
        };

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        assert!(matches!(
            runtime.process_message_mailbox(&store, group_id, page),
            Err(DurableMailboxError::KeyStore(KeyStoreError::BackendError))
        ));
        assert!(!runtime.is_ready());
        assert_eq!(
            load_stored_message(&store, group_id, &message_id),
            Err(KeyStoreError::NotFound)
        );
        assert_eq!(
            pending_message_acknowledgment(&store, group_id, device_id).unwrap(),
            None
        );
        runtime.reload(&store).unwrap();
        assert!(runtime.is_ready());
    }

    #[test]
    fn commit_checkpoint_and_ack_survive_restart() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, pending) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &key_package).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let entry = E2eeCommitMailboxEntry {
            epoch: pending.epoch,
            prior_epoch: pending.prior_epoch,
            committer_device_id: pending.committer_device_id.to_string(),
            commit_blob: pending.commit_blob,
            welcome_blob: pending.welcome_blob,
            created_at_unix: 10,
            expires_at_unix: 20,
        };
        let page = E2eeCommitMailboxResponse {
            next_after_epoch: Some(entry.epoch),
            commits: vec![entry],
        };
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[]).unwrap();
        let bob_device_id = bob.device_id();

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let batch = runtime
            .process_commit_mailbox(&store, group_id, alice_pin, page.clone())
            .unwrap();
        assert_eq!(batch.processed_epochs, vec![1]);
        let acknowledgment = batch.acknowledgment.unwrap();
        drop(runtime);

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(restarted.state.as_ref().unwrap().conversations.len(), 1);
        assert_eq!(
            pending_commit_acknowledgment(&store, group_id, bob_device_id).unwrap(),
            Some(acknowledgment.clone())
        );
        assert!(matches!(
            restarted.process_commit_mailbox(&store, group_id, alice_pin, page),
            Err(DurableMailboxError::PendingAcknowledgment)
        ));
        confirm_commit_acknowledgment(&store, group_id, &acknowledgment).unwrap();
        assert_eq!(
            pending_commit_acknowledgment(&store, group_id, bob_device_id).unwrap(),
            None
        );
    }

    #[test]
    fn invalid_message_timestamps_are_rejected_before_mls_state_changes() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
            ..
        } = joined_fixture();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let message_id = Ulid::new().to_string();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"bad timestamp")
            .unwrap();
        let mut entry = message_entry(message_id.clone(), encrypted);
        entry.expires_at_unix = entry.created_at_unix;
        let page = E2eeMailboxResponse {
            messages: vec![entry],
            next_after_message_id: Some(message_id),
        };
        let mut runtime = DurableMlsClient::load(&store).unwrap();

        assert_eq!(
            runtime
                .process_message_mailbox(&store, group_id, page)
                .err(),
            Some(DurableMailboxError::Conversation(
                ConversationError::InvalidMailboxPage
            ))
        );
        assert!(runtime.is_ready());
    }

    #[test]
    fn external_commit_recovery_is_isolated_accepted_and_restart_safe() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            mut bob_group,
            group_id,
        } = joined_fixture();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let before_recovery = alice_group
            .encrypt_application_message(&alice, b"before recovery")
            .unwrap();
        bob_group
            .decrypt_application_message(&bob, &before_recovery)
            .unwrap();
        let before_recovery_reply = bob_group
            .encrypt_application_message(&bob, b"before recovery reply")
            .unwrap();
        alice_group
            .decrypt_application_message(&alice, &before_recovery_reply)
            .unwrap();
        let update = alice_group.create_self_update(&alice).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let recovery_info = ExternalCommitRecoveryInfo::try_from(GroupInfoResponse {
            group_id: group_id.to_string(),
            epoch: update.epoch,
            suite_id: update.suite.as_u16(),
            group_info_blob: update.group_info_blob.clone().unwrap(),
        })
        .unwrap();

        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let recovery = runtime
            .prepare_external_commit_recovery(&store, alice_pin, &recovery_info)
            .unwrap();
        let pending = recovery.pending_commit();
        assert_eq!(pending.prior_epoch, update.epoch);
        assert_eq!(pending.epoch, update.epoch + 1);
        assert!(pending.welcome_blob.is_none());

        let encrypted_commit = crate::EncryptedGroupCommit {
            group_id,
            prior_epoch: pending.prior_epoch,
            epoch: pending.epoch,
            committer_device_id: pending.committer_device_id,
            commit_blob: pending.commit_blob.clone(),
        };
        let mut forged_hint = encrypted_commit.clone();
        forged_hint.committer_device_id = DeviceId::new();
        assert_eq!(
            alice_group.process_incoming_commit(&alice, &forged_hint),
            Err(ConversationError::MetadataMismatch)
        );
        assert_eq!(alice_group.epoch(), update.epoch);
        alice_group
            .process_incoming_commit(&alice, &encrypted_commit)
            .unwrap();
        runtime
            .confirm_external_commit_recovery(
                &store,
                recovery,
                &PostCommitResponse {
                    accepted: true,
                    epoch: update.epoch + 1,
                },
            )
            .unwrap();
        drop(runtime);

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"after external recovery")
            .unwrap();
        let state = restarted.state.as_mut().unwrap();
        let recovered = state
            .conversations
            .iter_mut()
            .find(|conversation| conversation.group_id() == group_id)
            .unwrap();
        let outcome = recovered
            .decrypt_application_message(&state.device, &encrypted)
            .unwrap();
        assert_eq!(
            outcome.ready_messages[0].plaintext,
            b"after external recovery"
        );
        let reply = recovered
            .encrypt_application_message(&state.device, b"reply after external recovery")
            .unwrap();
        let reply_outcome = alice_group
            .decrypt_application_message(&alice, &reply)
            .unwrap();
        assert_eq!(
            reply_outcome.ready_messages[0].plaintext,
            b"reply after external recovery"
        );
    }

    #[test]
    fn rejected_recovery_response_leaves_original_checkpoint_usable() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
        } = joined_fixture();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let update = alice_group.create_self_update(&alice).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let recovery_info = ExternalCommitRecoveryInfo::try_from(GroupInfoResponse {
            group_id: group_id.to_string(),
            epoch: update.epoch,
            suite_id: update.suite.as_u16(),
            group_info_blob: update.group_info_blob.clone().unwrap(),
        })
        .unwrap();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let recovery = runtime
            .prepare_external_commit_recovery(&store, alice_pin, &recovery_info)
            .unwrap();

        assert_eq!(
            runtime.confirm_external_commit_recovery(
                &store,
                recovery,
                &PostCommitResponse {
                    accepted: true,
                    epoch: update.epoch + 2,
                },
            ),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert!(runtime.is_ready());

        let state = runtime.state.as_mut().unwrap();
        state.conversations[0]
            .process_incoming_commit(
                &state.device,
                &crate::EncryptedGroupCommit {
                    group_id,
                    prior_epoch: update.prior_epoch,
                    epoch: update.epoch,
                    committer_device_id: update.committer_device_id,
                    commit_blob: update.commit_blob,
                },
            )
            .unwrap();
        assert_eq!(state.conversations[0].epoch(), update.epoch);
    }

    #[test]
    fn recovery_candidate_cannot_roll_back_same_epoch_mailbox_progress() {
        let JoinedFixture {
            alice,
            mut alice_group,
            bob,
            bob_group,
            group_id,
        } = joined_fixture();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"must not be rolled back")
            .unwrap();
        let update = alice_group.create_self_update(&alice).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let recovery_info = ExternalCommitRecoveryInfo::try_from(GroupInfoResponse {
            group_id: group_id.to_string(),
            epoch: update.epoch,
            suite_id: update.suite.as_u16(),
            group_info_blob: update.group_info_blob.unwrap(),
        })
        .unwrap();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let recovery = runtime
            .prepare_external_commit_recovery(&store, alice_pin, &recovery_info)
            .unwrap();

        let message_id = Ulid::new().to_string();
        let page = E2eeMailboxResponse {
            messages: vec![message_entry(message_id.clone(), encrypted)],
            next_after_message_id: Some(message_id),
        };
        let batch = runtime
            .process_message_mailbox(&store, group_id, page)
            .unwrap();
        assert_eq!(
            batch.ready_messages[0].plaintext,
            b"must not be rolled back"
        );

        assert_eq!(
            runtime.confirm_external_commit_recovery(
                &store,
                recovery,
                &PostCommitResponse {
                    accepted: true,
                    epoch: update.epoch + 1,
                },
            ),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert!(runtime.is_ready());
    }
}
