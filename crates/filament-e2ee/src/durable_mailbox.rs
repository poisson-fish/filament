//! Durable native coordination for MLS mailboxes.
//!
//! Mailbox processing consumes forward-secure MLS state. This module binds the
//! resulting checkpoint, authenticated plaintext history, and acknowledgment
//! outbox into one encrypted-store transaction. Network acknowledgments are
//! exposed only after that transaction commits and survive process restarts.

use std::collections::{HashMap, HashSet};

use filament_core::{CiphersuiteId, ConversationId, DeviceId, GroupId, ProposalId, UserId};
use filament_protocol::{
    AckE2eeCommitsRequest, AckE2eeMessagesRequest, AckE2eeProposalsRequest,
    ClaimKeyPackageResponse, CreateMlsConversationRequest, E2eeCommitMailboxResponse,
    E2eeMailboxResponse, E2eeProposalMailboxResponse, E2eeRetentionSeconds,
    MlsConversationProvisionResponse, MlsMembershipChange, PostCommitRequest, PostCommitResponse,
    PostMessageRequest, PostMessageResponse, MAX_COMMIT_BYTES, MAX_E2EE_COMMIT_ACK_BATCH_SIZE,
    MAX_E2EE_MESSAGE_ACK_BATCH_SIZE, MAX_E2EE_PROPOSAL_ACK_BATCH_SIZE,
    MAX_E2EE_PROPOSAL_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE,
    MAX_GROUP_INFO_BYTES, MAX_KEYPACKAGE_BYTES, MAX_PROPOSAL_BYTES, MAX_WELCOME_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use ulid::Ulid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    commit_mailbox::validate_page as validate_commit_page, persistence::encode_mls_client_state,
    process_commit_mailbox, process_group_commit_mailbox, process_message_mailbox,
    AuthenticatedMembershipChange, AuthenticatedMembershipChangeKind, ConversationAudience,
    ConversationError, DecryptedApplicationMessage, EncryptedChatEvent, EncryptedGroupCommit,
    ExternalCommitRecoveryInfo, ExternalGroupProposal, ExternalProposalAction, KeyStoreError,
    LocalKeyStore, MlsClientState, MlsConversation, PendingCommitRebase,
    PendingExternalCommitRecovery, PendingGroupCommit, PinnedUserIdentity, RejectedMailboxCommit,
    RejectedMailboxMessage, StoreKey, VersionedApplicationEvent, MAX_APPLICATION_PLAINTEXT_BYTES,
    MAX_STORE_VALUE_BYTES,
};

const HISTORY_RECORD_VERSION: u16 = 2;
const RETENTION_POLICY_VERSION: u16 = 1;
const OUTBOUND_COMMIT_RECORD_VERSION: u16 = 1;
const OUTBOUND_MESSAGE_RECORD_VERSION: u16 = 1;
const CONVERSATION_PROVISION_RECORD_VERSION: u16 = 1;
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
    /// An acceptance-gated local commit must be resolved before another proposal.
    #[error("an outbound MLS commit is already pending")]
    PendingOutboundCommit,
    /// A winning commit made the pending policy intent unsafe to retry.
    #[error("the outbound MLS commit intent was invalidated by the accepted epoch")]
    InvalidatedOutboundCommit,
    /// An encrypted application message must be resolved before the group epoch changes.
    #[error("an outbound MLS message is already pending")]
    PendingOutboundMessage,
    /// A restart-safe conversation bootstrap must be resolved first.
    #[error("an MLS conversation provision is already pending")]
    PendingConversationProvision,
    /// A direct-message MLS conversation already pins this peer.
    #[error("an MLS conversation for this peer already exists")]
    ConversationAlreadyExists,
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

/// A Delivery Service proposal result whose MLS effects and acknowledgment are durable.
pub struct DurableProposalMailboxBatch {
    /// Server proposal IDs authenticated and checkpointed by this call.
    pub processed_proposal_ids: Vec<String>,
    /// Durable request safe for the native network boundary to submit.
    pub acknowledgment: Option<AckE2eeProposalsRequest>,
    /// Exact acceptance-gated commit request staged by a non-target member.
    pub outbound_commit: Option<PostCommitRequest>,
    /// Whether this device authenticated its own removal and awaits a peer commit.
    pub awaiting_peer_commit: bool,
}

/// Durable result of authenticating the commit that won an epoch conflict.
pub struct DurableOutboundCommitRebase {
    /// Exact acknowledgment for the authenticated winning epoch.
    pub acknowledgment: AckE2eeCommitsRequest,
    /// Fresh acceptance-gated commit request, when the policy intent remains valid.
    pub outbound_commit: Option<PostCommitRequest>,
    /// Whether the winner itself already satisfied the pending intent.
    pub already_satisfied: bool,
    /// Whether the winner made the pending intent unsafe to retry.
    pub invalidated: bool,
}

/// Public routing metadata required by the native mailbox transport.
///
/// Root pins are public identity material, but remain native-only so an
/// untrusted UI cannot choose or alter the membership trust set used for
/// commit processing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailboxConversationRoute {
    pub group_id: GroupId,
    pub audience: ConversationAudience,
    pub participants: Vec<PinnedUserIdentity>,
}

/// One authenticated plaintext record loaded from encrypted native history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMailboxMessage {
    pub message_id: String,
    pub group_id: GroupId,
    pub created_at_unix: i64,
    /// Authenticated local deletion deadline, or `None` for retained history.
    pub expires_at_unix: Option<i64>,
    pub message: DecryptedApplicationMessage,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryRecordRef<'a> {
    version: u16,
    message_id: &'a str,
    group_id: String,
    created_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at_unix: Option<i64>,
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
    #[serde(default)]
    expires_at_unix: Option<i64>,
    sender_user_id: String,
    sender_device_id: String,
    generation: u64,
    plaintext: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionPolicyRecord {
    version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retention_secs: Option<E2eeRetentionSeconds>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutboundCommitRecord {
    version: u16,
    accepted: bool,
    #[serde(default)]
    invalidated: bool,
    request: PostCommitRequest,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutboundMessageRecord {
    version: u16,
    accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<PostMessageResponse>,
    request: PostMessageRequest,
    generation: u64,
    plaintext: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationProvisionRecord {
    version: u16,
    accepted: bool,
    base_checkpoint_sha256: [u8; 32],
    request: CreateMlsConversationRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<MlsConversationProvisionResponse>,
}

impl Drop for OutboundMessageRecord {
    fn drop(&mut self) {
        self.plaintext.zeroize();
    }
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

    /// Return a bounded native-only snapshot of locally authenticated mailbox
    /// routes.
    ///
    /// # Errors
    /// Rejects unavailable state or a checkpoint whose pinned audience no
    /// longer satisfies the direct-message/group-message invariants.
    pub fn mailbox_routes(&self) -> Result<Vec<MailboxConversationRoute>, DurableMailboxError> {
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        let own_user_id = state.device.user_id();
        state
            .conversations
            .iter()
            .map(|conversation| {
                let metadata = conversation.persistence_metadata();
                let own_pin_count = metadata
                    .pinned_roots
                    .iter()
                    .filter(|(user_id, root)| {
                        *user_id == own_user_id && root == state.device.root_key_public()
                    })
                    .count();
                let participants = metadata
                    .pinned_roots
                    .into_iter()
                    .filter(|(user_id, _)| *user_id != own_user_id)
                    .map(|(user_id, root_key_pub)| PinnedUserIdentity::new(user_id, root_key_pub))
                    .collect::<Vec<_>>();
                let valid_audience = match metadata.audience {
                    ConversationAudience::DirectMessage => participants.len() == 1,
                    ConversationAudience::GroupDm => (1..=99).contains(&participants.len()),
                };
                if own_pin_count != 1 || !valid_audience {
                    return Err(ConversationError::MetadataMismatch.into());
                }
                Ok(MailboxConversationRoute {
                    group_id: metadata.group_id,
                    audience: metadata.audience,
                    participants,
                })
            })
            .collect()
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

    /// Stage a new two-user MLS conversation behind a durable retry outbox.
    ///
    /// The claimed KeyPackage is authenticated against `peer`, including the
    /// server-routed device ID. The live MLS state remains unchanged. An
    /// isolated candidate checkpoint and exact provisioning request are
    /// inserted atomically before the request may cross the network boundary.
    ///
    /// # Errors
    /// Rejects duplicate peers/groups, malformed claims, pending local work,
    /// untrusted KeyPackages, or any encrypted-store failure.
    pub fn prepare_direct_message_provision(
        &self,
        store: &dyn LocalKeyStore,
        conversation_id: ConversationId,
        group_id: GroupId,
        peer: PinnedUserIdentity,
        claimed: &ClaimKeyPackageResponse,
    ) -> Result<CreateMlsConversationRequest, DurableMailboxError> {
        let provision_key = StoreKey::pending_conversation_provision();
        let checkpoint_key = StoreKey::pending_conversation_checkpoint();
        if store.exists(&provision_key)? || store.exists(&checkpoint_key)? {
            return Err(DurableMailboxError::PendingConversationProvision);
        }
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        if peer.user_id == state.device.user_id()
            || state
                .conversations
                .iter()
                .any(|conversation| conversation.group_id() == group_id)
        {
            return Err(DurableMailboxError::ConversationAlreadyExists);
        }
        for conversation in &state.conversations {
            let metadata = conversation.persistence_metadata();
            if metadata.audience == ConversationAudience::DirectMessage
                && metadata
                    .pinned_roots
                    .iter()
                    .any(|(user_id, _)| *user_id == peer.user_id)
            {
                return Err(DurableMailboxError::ConversationAlreadyExists);
            }
        }
        if has_any_pending_group_work(store, state)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let welcome_device_id = DeviceId::try_from(claimed.device_id.clone())
            .map_err(|_| ConversationError::MetadataMismatch)?;
        if claimed.key_package_blob.is_empty()
            || claimed.key_package_blob.len() > MAX_KEYPACKAGE_BYTES
            || crate::conversation::verified_keypackage_device(
                &state.device,
                &claimed.key_package_blob,
                peer,
            )? != welcome_device_id
        {
            return Err(ConversationError::MetadataMismatch.into());
        }

        let current_checkpoint = encode_state(state)?;
        let mut candidate = crate::persistence::clone_client_state(&current_checkpoint)?;
        let (conversation, pending) = MlsConversation::create_two_member(
            group_id,
            &candidate.device,
            peer,
            &claimed.key_package_blob,
        )?;
        if pending.prior_epoch != 0
            || pending.epoch != 1
            || pending.suite != CiphersuiteId::baseline()
            || pending.committer_device_id != candidate.device.device_id()
        {
            return Err(ConversationError::MetadataMismatch.into());
        }
        let welcome_blob = pending
            .welcome_blob
            .ok_or(ConversationError::MetadataMismatch)?;
        let group_info_blob = pending
            .group_info_blob
            .ok_or(ConversationError::MetadataMismatch)?;
        if pending.commit_blob.is_empty()
            || pending.commit_blob.len() > MAX_COMMIT_BYTES
            || welcome_blob.is_empty()
            || welcome_blob.len() > MAX_WELCOME_BYTES
            || group_info_blob.is_empty()
            || group_info_blob.len() > MAX_GROUP_INFO_BYTES
        {
            return Err(ConversationError::LimitExceeded.into());
        }
        let request = CreateMlsConversationRequest {
            conversation_id: conversation_id.to_string(),
            peer_user_id: peer.user_id.to_string(),
            group_id: group_id.to_string(),
            suite_id: pending.suite.as_u16(),
            committer_device_id: candidate.device.device_id().to_string(),
            welcome_device_id: welcome_device_id.to_string(),
            commit_blob: pending.commit_blob,
            welcome_blob,
            group_info_blob,
        };
        candidate.conversations.push(conversation);
        let candidate_checkpoint = encode_state(&candidate)?;
        let record = ConversationProvisionRecord {
            version: CONVERSATION_PROVISION_RECORD_VERSION,
            accepted: false,
            base_checkpoint_sha256: Sha256::digest(&current_checkpoint).into(),
            request: request.clone(),
            response: None,
        };
        store.store_batch_if_absent_or_equal(vec![
            (provision_key, encode_json(&record)?),
            (checkpoint_key, candidate_checkpoint),
        ])?;
        Ok(request)
    }

    /// Return the exact pending provisioning request, reconciling a completed
    /// local adoption marker after restart.
    ///
    /// # Errors
    /// Rejects torn, corrupt, or state-conflicting outboxes. An accepted marker
    /// is removed only when its checkpoint exactly equals the active one.
    pub fn pending_conversation_provision(
        &mut self,
        store: &dyn LocalKeyStore,
    ) -> Result<Option<CreateMlsConversationRequest>, DurableMailboxError> {
        let provision_key = StoreKey::pending_conversation_provision();
        let checkpoint_key = StoreKey::pending_conversation_checkpoint();
        let has_record = store.exists(&provision_key)?;
        let has_checkpoint = store.exists(&checkpoint_key)?;
        if has_record != has_checkpoint {
            return Err(KeyStoreError::InvalidValue.into());
        }
        if !has_record {
            return Ok(None);
        }
        let record = load_conversation_provision_record(store)?;
        let checkpoint = store.load(&checkpoint_key)?;
        let candidate = crate::persistence::decode_mls_client_state(&checkpoint)?;
        validate_conversation_provision_candidate(&record, &candidate)?;
        if !record.accepted {
            validate_conversation_provision_base(store, &record)?;
            return Ok(Some(record.request));
        }
        let current_checkpoint = store.load(&StoreKey::mls_client_state())?;
        if current_checkpoint.as_slice() != checkpoint.as_slice() {
            return Err(KeyStoreError::InvalidValue.into());
        }
        let current = crate::persistence::decode_mls_client_state(&current_checkpoint)?;
        validate_accepted_conversation_provision(&record, &current)?;
        self.state = Some(current);
        let removed = match store.remove_batch(&[provision_key, checkpoint_key]) {
            Ok(removed) => removed,
            Err(error) => {
                self.state = None;
                return Err(error.into());
            }
        };
        if removed != 2 {
            self.state = None;
            return Err(KeyStoreError::BackendError.into());
        }
        Ok(None)
    }

    /// Adopt an exact server-accepted conversation and durably close the
    /// response-loss window.
    ///
    /// # Errors
    /// Rejects substituted requests/responses or corrupt candidate state.
    /// Persistence uncertainty shuts the runtime down until reload.
    pub fn confirm_conversation_provision(
        &mut self,
        store: &dyn LocalKeyStore,
        submitted: &CreateMlsConversationRequest,
        response: &MlsConversationProvisionResponse,
    ) -> Result<(), DurableMailboxError> {
        let mut record = load_conversation_provision_record(store)?;
        if record.accepted || record.request != *submitted {
            return Err(ConversationError::MetadataMismatch.into());
        }
        validate_conversation_provision_base(store, &record)?;
        validate_conversation_provision_response(submitted, response)?;
        let checkpoint_key = StoreKey::pending_conversation_checkpoint();
        let checkpoint = store.load(&checkpoint_key)?;
        let mut candidate = crate::persistence::decode_mls_client_state(&checkpoint)?;
        validate_conversation_provision_candidate(&record, &candidate)?;
        let group_id = GroupId::try_from(submitted.group_id.clone())
            .map_err(|_| ConversationError::MetadataMismatch)?;
        let position = candidate
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
            .ok_or(DurableMailboxError::ConversationNotFound)?;
        candidate.conversations[position].accept_pending_commit(&candidate.device)?;
        let accepted_checkpoint = encode_state(&candidate)?;
        record.accepted = true;
        record.response = Some(response.clone());
        self.state = None;
        if let Err(error) = store.store_batch(vec![
            (StoreKey::mls_client_state(), accepted_checkpoint.clone()),
            (checkpoint_key.clone(), accepted_checkpoint),
            (
                StoreKey::pending_conversation_provision(),
                encode_json(&record)?,
            ),
        ]) {
            return Err(error.into());
        }
        self.state = Some(candidate);
        let removed = match store
            .remove_batch(&[StoreKey::pending_conversation_provision(), checkpoint_key])
        {
            Ok(removed) => removed,
            Err(error) => {
                self.state = None;
                return Err(error.into());
            }
        };
        if removed != 2 {
            self.state = None;
            return Err(KeyStoreError::BackendError.into());
        }
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
            || store.exists(&outbound_message_key(recovery.group_id)?)?
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

    /// Prepare an isolated external-commit recovery candidate for a group DM.
    ///
    /// Existing acknowledgment outboxes must be drained and `participants`
    /// must exactly match the native checkpoint's pinned roots other than the
    /// local user. The live state remains unchanged until confirmation.
    ///
    /// # Errors
    /// Returns a durability or fail-closed MLS validation error.
    pub fn prepare_group_external_commit_recovery(
        &self,
        store: &dyn LocalKeyStore,
        participants: &[PinnedUserIdentity],
        recovery: &ExternalCommitRecoveryInfo,
    ) -> Result<PendingExternalCommitRecovery, DurableMailboxError> {
        if store.exists(&message_ack_key(recovery.group_id)?)?
            || store.exists(&commit_ack_key(recovery.group_id)?)?
            || store.exists(&outbound_message_key(recovery.group_id)?)?
        {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        state
            .prepare_group_external_commit_recovery(participants, recovery)
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

    /// Authenticate and durably checkpoint at most one Delivery Service proposal.
    ///
    /// Processing one record at a time bounds native work and ensures a
    /// proposal-derived commit is ordered before any later proposal. The
    /// checkpoint, proposal acknowledgment, and exact outbound commit request
    /// are persisted atomically. Only the pinned Delivery Service external
    /// sender at index zero is accepted by the underlying MLS conversation.
    ///
    /// # Errors
    /// Rejects malformed pages, member-authored proposals, stale routing hints,
    /// unsupported audiences, or any existing acknowledgment/commit outbox.
    pub fn process_proposal_mailbox(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        page: E2eeProposalMailboxResponse,
    ) -> Result<DurableProposalMailboxBatch, DurableMailboxError> {
        validate_proposal_page(&page)?;
        if store.exists(&proposal_ack_key(group_id)?)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        if store.exists(&outbound_message_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundMessage);
        }
        if store.exists(&outbound_commit_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundCommit);
        }
        let Some(entry) = page.proposals.into_iter().next() else {
            return Ok(DurableProposalMailboxBatch {
                processed_proposal_ids: Vec::new(),
                acknowledgment: None,
                outbound_commit: None,
                awaiting_peer_commit: false,
            });
        };
        if entry.proposer_device_id.is_some() || entry.external_sender_index != Some(0) {
            return Err(ConversationError::UnexpectedMembership.into());
        }
        let proposal_id = ProposalId::try_from(entry.proposal_id.clone())
            .map_err(|_| ConversationError::InvalidMailboxPage)?;
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let Some(position) = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
        else {
            self.state = Some(state);
            return Err(DurableMailboxError::ConversationNotFound);
        };
        let proposal = ExternalGroupProposal {
            group_id,
            epoch: entry.epoch,
            proposal_blob: entry.proposal_blob,
        };
        let action = match state.conversations[position]
            .process_external_remove_proposal(&state.device, &proposal)
        {
            Ok(action) => action,
            Err(error) => {
                self.state = Some(state);
                return Err(error.into());
            }
        };
        let acknowledgment = AckE2eeProposalsRequest {
            device_id: state.device.device_id().to_string(),
            proposal_ids: vec![proposal_id.to_string()],
        };
        let (outbound_commit, awaiting_peer_commit) = match action {
            ExternalProposalAction::Commit {
                commit,
                removed_leaf,
            } => {
                let request = PostCommitRequest {
                    epoch: commit.epoch,
                    prior_epoch: commit.prior_epoch,
                    committer_device_id: commit.committer_device_id.to_string(),
                    commit_blob: commit.commit_blob,
                    welcome_blob: None,
                    welcome_device_id: None,
                    group_info_blob: commit.group_info_blob,
                    membership_change: Some(MlsMembershipChange::Remove {
                        leaves: vec![removed_leaf],
                    }),
                };
                validate_outbound_commit_request(state.device.device_id(), &request)?;
                (Some(request), false)
            }
            ExternalProposalAction::AwaitingPeerCommit => (None, true),
        };

        let mut entries = vec![
            (StoreKey::mls_client_state(), encode_state(&state)?),
            (proposal_ack_key(group_id)?, encode_json(&acknowledgment)?),
        ];
        if let Some(request) = &outbound_commit {
            entries.push((
                outbound_commit_key(group_id)?,
                encode_json(&OutboundCommitRecord {
                    version: OUTBOUND_COMMIT_RECORD_VERSION,
                    accepted: false,
                    invalidated: false,
                    request: request.clone(),
                })?,
            ));
        }
        if let Err(error) = store.store_batch(entries) {
            return Err(error.into());
        }
        self.state = Some(state);
        Ok(DurableProposalMailboxBatch {
            processed_proposal_ids: acknowledgment.proposal_ids.clone(),
            acknowledgment: Some(acknowledgment),
            outbound_commit,
            awaiting_peer_commit,
        })
    }

    /// Return the exact durable commit request that must be retried.
    ///
    /// An accepted marker left behind after a successful atomic checkpoint is
    /// removed only after verifying that the restored MLS epoch matches it.
    ///
    /// # Errors
    /// Rejects corrupt, cross-group/device, or checkpoint-mismatched outboxes.
    pub fn pending_outbound_commit(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
    ) -> Result<Option<PostCommitRequest>, DurableMailboxError> {
        let Some(record) = load_outbound_commit_record(store, group_id)? else {
            return Ok(None);
        };
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        validate_outbound_commit_request(state.device.device_id(), &record.request)?;
        if record.invalidated {
            return Err(DurableMailboxError::InvalidatedOutboundCommit);
        }
        let conversation = state
            .conversations
            .iter()
            .find(|conversation| conversation.group_id() == group_id)
            .ok_or(DurableMailboxError::ConversationNotFound)?;
        if record.accepted {
            if conversation.epoch() != record.request.epoch {
                return Err(ConversationError::MetadataMismatch.into());
            }
            let removed = store.remove_batch(&[outbound_commit_key(group_id)?])?;
            if removed != 1 {
                return Err(KeyStoreError::InvalidValue.into());
            }
            return Ok(None);
        }
        if conversation.epoch() != record.request.prior_epoch {
            return Err(ConversationError::MetadataMismatch.into());
        }
        Ok(Some(record.request))
    }

    /// Merge an exact retry-safe outbound commit after server acceptance.
    ///
    /// The merged checkpoint and an accepted outbox marker are stored
    /// atomically. A crash before marker cleanup is reconciled by
    /// [`Self::pending_outbound_commit`] after restart.
    ///
    /// # Errors
    /// Rejects substituted requests/responses or local state that no longer
    /// matches the pending epoch. An uncertain checkpoint write shuts down the
    /// runtime until reload.
    pub fn confirm_outbound_commit(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        submitted: &PostCommitRequest,
        response: &PostCommitResponse,
    ) -> Result<(), DurableMailboxError> {
        let record = load_outbound_commit_record(store, group_id)?
            .ok_or(DurableMailboxError::PendingOutboundCommit)?;
        if record.accepted
            || &record.request != submitted
            || !response.accepted
            || response.epoch != submitted.epoch
        {
            return Err(ConversationError::MetadataMismatch.into());
        }
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        validate_outbound_commit_request(state.device.device_id(), submitted)?;
        let Some(position) = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
        else {
            self.state = Some(state);
            return Err(DurableMailboxError::ConversationNotFound);
        };
        if state.conversations[position].epoch() != submitted.prior_epoch {
            self.state = Some(state);
            return Err(ConversationError::MetadataMismatch.into());
        }
        if let Err(error) = state.conversations[position].accept_pending_commit(&state.device) {
            self.state = Some(state);
            return Err(error.into());
        }
        let accepted = OutboundCommitRecord {
            version: OUTBOUND_COMMIT_RECORD_VERSION,
            accepted: true,
            invalidated: false,
            request: submitted.clone(),
        };
        if let Err(error) = store.store_batch(vec![
            (StoreKey::mls_client_state(), encode_state(&state)?),
            (outbound_commit_key(group_id)?, encode_json(&accepted)?),
        ]) {
            return Err(error.into());
        }
        self.state = Some(state);
        let removed = store.remove_batch(&[outbound_commit_key(group_id)?])?;
        if removed != 1 {
            return Err(KeyStoreError::InvalidValue.into());
        }
        Ok(())
    }

    /// Authenticate the commit that won a Delivery Service epoch conflict and
    /// durably rebase the pending policy intent.
    ///
    /// Only the first commit in the bounded mailbox page is consumed. Its MLS
    /// transition must match the server's untrusted membership routing hint.
    /// The winning checkpoint, commit acknowledgment, and replacement
    /// outbound request are stored atomically before any network action may
    /// continue.
    ///
    /// # Errors
    /// Rejects missing or non-competing winners, routing mismatches, unsafe
    /// rebases, corrupt outboxes, and any uncertain encrypted-store write.
    pub fn rebase_outbound_commit(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        page: E2eeCommitMailboxResponse,
    ) -> Result<DurableOutboundCommitRebase, DurableMailboxError> {
        validate_commit_page(&page)?;
        if store.exists(&outbound_message_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundMessage);
        }
        if store.exists(&commit_ack_key(group_id)?)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        let record = load_outbound_commit_record(store, group_id)?
            .ok_or(DurableMailboxError::PendingOutboundCommit)?;
        if record.accepted || record.invalidated {
            return Err(DurableMailboxError::PendingOutboundCommit);
        }
        let entry = page
            .commits
            .into_iter()
            .next()
            .ok_or(ConversationError::InvalidMailboxPage)?;
        if entry.prior_epoch != record.request.prior_epoch
            || entry.epoch != record.request.epoch
            || entry.welcome_blob.is_some()
        {
            return Err(ConversationError::MetadataMismatch.into());
        }

        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let committer_device_id = DeviceId::try_from(entry.committer_device_id.clone())
            .map_err(|_| ConversationError::MetadataMismatch)?;
        if committer_device_id == state.device.device_id() {
            self.state = Some(state);
            return Err(ConversationError::MetadataMismatch.into());
        }
        let Some(position) = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
        else {
            self.state = Some(state);
            return Err(DurableMailboxError::ConversationNotFound);
        };
        let winner = EncryptedGroupCommit {
            group_id,
            prior_epoch: entry.prior_epoch,
            epoch: entry.epoch,
            committer_device_id,
            commit_blob: entry.commit_blob.clone(),
        };
        let (outcome, membership_change) = state.conversations[position]
            .rebase_pending_commit_with_membership(&state.device, &winner)?;
        if validate_winner_membership_routing(
            entry.membership_change.as_ref(),
            membership_change.as_ref(),
        )
        .is_err()
        {
            // MLS state has already consumed the winner. Do not expose that
            // uncheckpointed state after hostile routing metadata; reload the
            // last complete encrypted checkpoint before retrying.
            return Err(ConversationError::MetadataMismatch.into());
        }

        let acknowledgment = AckE2eeCommitsRequest {
            device_id: state.device.device_id().to_string(),
            epochs: vec![entry.epoch],
        };
        let (outbound_commit, accepted, invalidated) = match outcome {
            PendingCommitRebase::Rebased(pending) => {
                let request =
                    rebased_remove_request(&pending, record.request.membership_change.clone())?;
                validate_outbound_commit_request(state.device.device_id(), &request)?;
                (Some(request), false, false)
            }
            PendingCommitRebase::AlreadySatisfied => (None, true, false),
            PendingCommitRebase::Invalidated => (None, false, true),
        };
        let replacement = OutboundCommitRecord {
            version: OUTBOUND_COMMIT_RECORD_VERSION,
            accepted,
            invalidated,
            request: outbound_commit.clone().unwrap_or(record.request),
        };
        if let Err(error) = store.store_batch(vec![
            (StoreKey::mls_client_state(), encode_state(&state)?),
            (commit_ack_key(group_id)?, encode_json(&acknowledgment)?),
            (outbound_commit_key(group_id)?, encode_json(&replacement)?),
        ]) {
            return Err(error.into());
        }
        self.state = Some(state);
        Ok(DurableOutboundCommitRebase {
            acknowledgment,
            outbound_commit,
            already_satisfied: accepted,
            invalidated,
        })
    }

    /// Encrypt and atomically checkpoint one exact retry-safe chat event.
    ///
    /// The advanced MLS sender ratchet, authenticated plaintext, and opaque
    /// transport request are committed in one encrypted-store transaction
    /// before network submission. Only the exact durable ciphertext may be
    /// retried after response loss.
    ///
    /// # Errors
    /// Rejects unknown groups, pending acknowledgments/commits/messages,
    /// retention-policy mismatches, invalid events, and uncertain writes.
    pub fn prepare_outbound_message(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        event: &crate::VersionedApplicationEvent,
    ) -> Result<PostMessageRequest, DurableMailboxError> {
        if store.exists(&outbound_message_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundMessage);
        }
        if store.exists(&message_ack_key(group_id)?)?
            || store.exists(&commit_ack_key(group_id)?)?
            || store.exists(&proposal_ack_key(group_id)?)?
        {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        if store.exists(&outbound_commit_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundCommit);
        }
        if event.retention_secs != load_disappearing_timer(store, group_id)? {
            return Err(ConversationError::MetadataMismatch.into());
        }

        let plaintext = Zeroizing::new(event.encode()?);
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let Some(position) = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id)
        else {
            self.state = Some(state);
            return Err(DurableMailboxError::ConversationNotFound);
        };
        let generation = state.conversations[position]
            .persistence_metadata()
            .outbound_generation;
        let encrypted = match state.conversations[position]
            .encrypt_application_message(&state.device, &plaintext)
        {
            Ok(encrypted) => encrypted,
            Err(error) => {
                self.state = Some(state);
                return Err(error.into());
            }
        };
        let request = PostMessageRequest {
            epoch: encrypted.epoch,
            suite_id: encrypted.suite.as_u16(),
            sender_device_id: encrypted.sender_device_id.to_string(),
            retention_secs: event.retention_secs,
            message_blob: encrypted.message_blob,
        };
        let record = OutboundMessageRecord {
            version: OUTBOUND_MESSAGE_RECORD_VERSION,
            accepted: false,
            response: None,
            request: request.clone(),
            generation,
            plaintext: plaintext.to_vec(),
        };
        if let Err(error) = store.store_batch(vec![
            (StoreKey::mls_client_state(), encode_state(&state)?),
            (outbound_message_key(group_id)?, encode_json(&record)?),
        ]) {
            return Err(error.into());
        }
        self.state = Some(state);
        Ok(request)
    }

    /// Return the exact durable message request that must be retried.
    ///
    /// An accepted marker left after local-history persistence is reconciled
    /// and removed only after the authenticated history record is verified.
    ///
    /// # Errors
    /// Rejects corrupt, cross-device/group, or checkpoint-mismatched outboxes.
    pub fn pending_outbound_message(
        &self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
    ) -> Result<Option<PostMessageRequest>, DurableMailboxError> {
        let Some(record) = load_outbound_message_record(store, group_id)? else {
            return Ok(None);
        };
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        validate_outbound_message_record(state, group_id, &record)?;
        if let Some(response) = &record.response {
            validate_outbound_message_history(store, state, group_id, &record, response)?;
            let removed = store.remove_batch(&[outbound_message_key(group_id)?])?;
            if removed != 1 {
                return Err(KeyStoreError::InvalidValue.into());
            }
            return Ok(None);
        }
        Ok(Some(record.request.clone()))
    }

    /// Persist the sender's authenticated local history after exact acceptance.
    ///
    /// History, a response-bearing accepted marker, and any authenticated
    /// disappearing-timer update are stored atomically. A crash before marker
    /// cleanup is reconciled by [`Self::pending_outbound_message`].
    ///
    /// # Errors
    /// Rejects substituted requests/responses, invalid timestamps/IDs, and
    /// checkpoint mismatches without clearing the durable retry request.
    pub fn confirm_outbound_message(
        &self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        submitted: &PostMessageRequest,
        response: &PostMessageResponse,
    ) -> Result<(), DurableMailboxError> {
        let mut record = load_outbound_message_record(store, group_id)?
            .ok_or(DurableMailboxError::PendingOutboundMessage)?;
        if record.accepted || record.response.is_some() || &record.request != submitted {
            return Err(ConversationError::MetadataMismatch.into());
        }
        let state = self
            .state
            .as_ref()
            .ok_or(DurableMailboxError::Unavailable)?;
        validate_outbound_message_record(state, group_id, &record)?;
        validate_message_response(response)?;
        let stored = outbound_stored_message(
            group_id,
            state.device.user_id(),
            state.device.device_id(),
            &record,
            response,
        )?;
        let history = history_storage_entry(&stored)?;
        record.accepted = true;
        record.response = Some(response.clone());
        let mut entries = vec![
            history,
            (outbound_message_key(group_id)?, encode_json(&record)?),
        ];
        if let EncryptedChatEvent::SetDisappearingTimer { retention_secs } =
            VersionedApplicationEvent::decode(&record.plaintext)?.event
        {
            entries.push((
                retention_policy_key(group_id)?,
                encode_json(&RetentionPolicyRecord {
                    version: RETENTION_POLICY_VERSION,
                    retention_secs,
                })?,
            ));
        }
        store.store_batch(entries)?;
        let removed = store.remove_batch(&[outbound_message_key(group_id)?])?;
        if removed != 1 {
            return Err(KeyStoreError::InvalidValue.into());
        }
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
        self.process_message_mailbox_at(store, group_id, page, unix_now())
    }

    /// Process one message page using an explicit native clock value.
    ///
    /// This exists for deterministic host integration and tests. Authenticated
    /// records already past their local deadline are consumed and acknowledged
    /// but never persisted or released as plaintext.
    ///
    /// # Errors
    /// Returns the same durability and validation errors as
    /// [`Self::process_message_mailbox`], plus invalid native clock values.
    pub fn process_message_mailbox_at(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        page: E2eeMailboxResponse,
        now_unix: i64,
    ) -> Result<DurableMessageMailboxBatch, DurableMailboxError> {
        if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
            return Err(KeyStoreError::InvalidValue.into());
        }
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
        let mut batch = match process_message_mailbox(
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

        let mut generation_expiries = stored_generation_expiries(store, group_id)?;
        for authenticated in &batch.authenticated_messages {
            generation_expiries.insert(
                (
                    authenticated.message.sender_device_id,
                    authenticated.message.generation,
                ),
                authenticated_local_expiry(authenticated)?,
            );
        }
        batch.ready_messages.retain(|message| {
            generation_expiries
                .get(&(message.sender_device_id, message.generation))
                .is_some_and(|expires_at| expires_at.is_none_or(|expiry| expiry > now_unix))
        });

        if let Some(acknowledgment) = &batch.pending_acknowledgment {
            let entries =
                message_durability_entries(&state, group_id, &batch, acknowledgment, now_unix)?;
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
        if store.exists(&outbound_message_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundMessage);
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

    /// Process an ordered group-DM commit page against an exact locally pinned
    /// participant set and atomically persist its successful prefix.
    ///
    /// # Errors
    /// Returns fail-closed mailbox, audience, MLS, or durability errors.
    pub fn process_group_commit_mailbox(
        &mut self,
        store: &dyn LocalKeyStore,
        group_id: GroupId,
        participants: &[PinnedUserIdentity],
        page: E2eeCommitMailboxResponse,
    ) -> Result<DurableCommitMailboxBatch, DurableMailboxError> {
        if store.exists(&commit_ack_key(group_id)?)? {
            return Err(DurableMailboxError::PendingAcknowledgment);
        }
        if store.exists(&outbound_message_key(group_id)?)? {
            return Err(DurableMailboxError::PendingOutboundMessage);
        }
        let mut state = self.state.take().ok_or(DurableMailboxError::Unavailable)?;
        let existing_position = state
            .conversations
            .iter()
            .position(|conversation| conversation.group_id() == group_id);
        let mut conversation =
            existing_position.map(|position| state.conversations.remove(position));
        let batch = match process_group_commit_mailbox(
            &mut conversation,
            &state.device,
            group_id,
            participants,
            page,
        ) {
            Ok(batch) => batch,
            Err(error) => {
                if let Some(conversation) = conversation {
                    insert_conversation(&mut state.conversations, existing_position, conversation);
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

/// Load a restart-safe proposal acknowledgment from the native outbox.
///
/// # Errors
/// Rejects corrupt, cross-device, duplicate, oversized, or non-canonical data.
pub fn pending_proposal_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    expected_device_id: DeviceId,
) -> Result<Option<AckE2eeProposalsRequest>, KeyStoreError> {
    let key = proposal_ack_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let acknowledgment: AckE2eeProposalsRequest =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    validate_proposal_ack(&acknowledgment, expected_device_id)?;
    Ok(Some(acknowledgment))
}

/// Remove a proposal acknowledgment only after a successful server response.
///
/// # Errors
/// Rejects a request that does not exactly match the durable outbox record.
pub fn confirm_proposal_acknowledgment(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    submitted: &AckE2eeProposalsRequest,
) -> Result<(), KeyStoreError> {
    confirm_ack(store, &proposal_ack_key(group_id)?, submitted)
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
    load_stored_message_at(store, group_id, message_id, unix_now())
}

/// Read one message while enforcing its authenticated local deletion deadline.
/// Expired ciphertext is atomically removed before `NotFound` is returned.
///
/// # Errors
/// Rejects invalid identifiers or records and propagates encrypted-store
/// failures without returning expired plaintext.
pub fn load_stored_message_at(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
    message_id: &str,
    now_unix: i64,
) -> Result<StoredMailboxMessage, KeyStoreError> {
    validate_ulid(message_id)?;
    let key = history_key(group_id, message_id)?;
    let encoded = store.load(&key)?;
    let message = decode_stored_message(group_id, message_id, &encoded)?;
    if message
        .expires_at_unix
        .is_some_and(|expires_at| expires_at <= now_unix)
    {
        store.remove_batch(&[key])?;
        return Err(KeyStoreError::NotFound);
    }
    Ok(message)
}

pub(crate) fn decode_stored_message(
    group_id: GroupId,
    message_id: &str,
    encoded: &[u8],
) -> Result<StoredMailboxMessage, KeyStoreError> {
    validate_ulid(message_id)?;
    if encoded.len() > MAX_LOCAL_HISTORY_RECORD_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut record: HistoryRecord =
        serde_json::from_slice(encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    if !matches!(record.version, 1 | HISTORY_RECORD_VERSION)
        || record.message_id != message_id
        || !(0..=MAX_UNIX_TIMESTAMP).contains(&record.created_at_unix)
        || record.plaintext.is_empty()
        || record.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
        || record.expires_at_unix.is_some_and(|expires_at| {
            expires_at <= record.created_at_unix || expires_at > MAX_UNIX_TIMESTAMP
        })
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
        expires_at_unix: record.expires_at_unix,
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
    now_unix: i64,
) -> Result<Vec<(StoreKey, Vec<u8>)>, DurableMailboxError> {
    if batch.authenticated_messages.len() != acknowledgment.message_ids.len() {
        return Err(KeyStoreError::InvalidValue.into());
    }
    let mut entries = Vec::with_capacity(batch.authenticated_messages.len() + 3);
    entries.push((StoreKey::mls_client_state(), encode_state(state)?));
    let mut retention_update = None;
    for authenticated in &batch.authenticated_messages {
        let message = &authenticated.message;
        let application = crate::VersionedApplicationEvent::decode(&message.plaintext).ok();
        let expires_at_unix = authenticated_local_expiry(authenticated)?;
        if expires_at_unix.is_some_and(|expires_at| expires_at <= now_unix) {
            continue;
        }
        if let Some(crate::EncryptedChatEvent::SetDisappearingTimer { retention_secs }) =
            application.as_ref().map(|event| &event.event)
        {
            retention_update = Some(*retention_secs);
        }
        let record = HistoryRecordRef {
            version: HISTORY_RECORD_VERSION,
            message_id: &authenticated.message_id,
            group_id: group_id.to_string(),
            created_at_unix: authenticated.created_at_unix,
            expires_at_unix,
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
    if let Some(retention_secs) = retention_update {
        entries.push((
            retention_policy_key(group_id)?,
            encode_json(&RetentionPolicyRecord {
                version: RETENTION_POLICY_VERSION,
                retention_secs,
            })?,
        ));
    }
    entries.push((message_ack_key(group_id)?, encode_json(acknowledgment)?));
    Ok(entries)
}

fn authenticated_local_expiry(
    authenticated: &crate::AuthenticatedMailboxMessage,
) -> Result<Option<i64>, KeyStoreError> {
    let retention_secs = crate::VersionedApplicationEvent::decode(&authenticated.message.plaintext)
        .ok()
        .and_then(|event| event.retention_secs);
    retention_secs
        .map(|duration| {
            authenticated
                .created_at_unix
                .checked_add(
                    i64::try_from(duration.as_u64()).map_err(|_| KeyStoreError::InvalidValue)?,
                )
                .filter(|expires_at| *expires_at <= MAX_UNIX_TIMESTAMP)
                .ok_or(KeyStoreError::InvalidValue)
        })
        .transpose()
}

fn stored_generation_expiries(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
) -> Result<HashMap<(DeviceId, u64), Option<i64>>, KeyStoreError> {
    let mut expiries = HashMap::new();
    let prefix = format!("history:{group_id}:");
    for key in store.list_keys()? {
        let Some(message_id) = key.as_str().strip_prefix(&prefix) else {
            continue;
        };
        let encoded = store.load(&key)?;
        let stored = decode_stored_message(group_id, message_id, &encoded)?;
        if expiries
            .insert(
                (stored.message.sender_device_id, stored.message.generation),
                stored.expires_at_unix,
            )
            .is_some()
        {
            return Err(KeyStoreError::InvalidValue);
        }
    }
    Ok(expiries)
}

pub(crate) fn history_storage_entry(
    message: &StoredMailboxMessage,
) -> Result<(StoreKey, Vec<u8>), KeyStoreError> {
    validate_ulid(&message.message_id)?;
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&message.created_at_unix)
        || message.expires_at_unix.is_some_and(|expires_at| {
            expires_at <= message.created_at_unix || expires_at > MAX_UNIX_TIMESTAMP
        })
        || message.message.plaintext.is_empty()
        || message.message.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
    {
        return Err(KeyStoreError::InvalidValue);
    }
    let record = HistoryRecordRef {
        version: HISTORY_RECORD_VERSION,
        message_id: &message.message_id,
        group_id: message.group_id.to_string(),
        created_at_unix: message.created_at_unix,
        expires_at_unix: message.expires_at_unix,
        sender_user_id: message.message.sender_user_id.to_string(),
        sender_device_id: message.message.sender_device_id.to_string(),
        generation: message.message.generation,
        plaintext: &message.message.plaintext,
    };
    let encoded = encode_json(&record)?;
    if encoded.len() > MAX_LOCAL_HISTORY_RECORD_BYTES {
        return Err(KeyStoreError::LimitExceeded);
    }
    Ok((history_key(message.group_id, &message.message_id)?, encoded))
}

fn encode_state(state: &MlsClientState) -> Result<Vec<u8>, KeyStoreError> {
    let conversations = state.conversations.iter().collect::<Vec<_>>();
    encode_mls_client_state(&state.device, &conversations).map(|encoded| encoded.to_vec())
}

fn has_any_pending_group_work(
    store: &dyn LocalKeyStore,
    state: &MlsClientState,
) -> Result<bool, KeyStoreError> {
    for conversation in &state.conversations {
        let group_id = conversation.group_id();
        if store.exists(&message_ack_key(group_id)?)?
            || store.exists(&commit_ack_key(group_id)?)?
            || store.exists(&proposal_ack_key(group_id)?)?
            || store.exists(&outbound_commit_key(group_id)?)?
            || store.exists(&outbound_message_key(group_id)?)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_conversation_provision_record(
    store: &dyn LocalKeyStore,
) -> Result<ConversationProvisionRecord, KeyStoreError> {
    let encoded = store.load(&StoreKey::pending_conversation_provision())?;
    if encoded.is_empty() || encoded.len() > MAX_STORE_VALUE_BYTES {
        return Err(KeyStoreError::InvalidValue);
    }
    serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)
}

fn validate_conversation_provision_base(
    store: &dyn LocalKeyStore,
    record: &ConversationProvisionRecord,
) -> Result<(), DurableMailboxError> {
    let current = store.load(&StoreKey::mls_client_state())?;
    let digest: [u8; 32] = Sha256::digest(&current).into();
    if digest != record.base_checkpoint_sha256 {
        return Err(ConversationError::MetadataMismatch.into());
    }
    Ok(())
}

fn validate_conversation_provision_candidate(
    record: &ConversationProvisionRecord,
    candidate: &MlsClientState,
) -> Result<(), DurableMailboxError> {
    let request = &record.request;
    let conversation_id = ConversationId::try_from(request.conversation_id.clone())
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let peer_user_id = UserId::try_from(request.peer_user_id.clone())
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let group_id = GroupId::try_from(request.group_id.clone())
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let committer_device_id = DeviceId::try_from(request.committer_device_id.clone())
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let welcome_device_id = DeviceId::try_from(request.welcome_device_id.clone())
        .map_err(|_| ConversationError::MetadataMismatch)?;
    if record.version != CONVERSATION_PROVISION_RECORD_VERSION
        || conversation_id.to_string() != request.conversation_id
        || peer_user_id == candidate.device.user_id()
        || committer_device_id != candidate.device.device_id()
        || welcome_device_id == committer_device_id
        || request.suite_id != CiphersuiteId::baseline().as_u16()
        || request.commit_blob.is_empty()
        || request.commit_blob.len() > MAX_COMMIT_BYTES
        || request.welcome_blob.is_empty()
        || request.welcome_blob.len() > MAX_WELCOME_BYTES
        || request.group_info_blob.is_empty()
        || request.group_info_blob.len() > MAX_GROUP_INFO_BYTES
        || record.accepted != record.response.is_some()
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    let matches = candidate
        .conversations
        .iter()
        .filter(|conversation| conversation.group_id() == group_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(ConversationError::MetadataMismatch.into());
    }
    let metadata = matches[0].persistence_metadata();
    let expected_epoch = u64::from(record.accepted);
    if metadata.audience != ConversationAudience::DirectMessage
        || metadata.active != record.accepted
        || metadata.epoch != expected_epoch
        || metadata.own_device_id != candidate.device.device_id()
        || metadata.pinned_roots.len() != 2
        || !metadata.pinned_roots.iter().any(|(user_id, root)| {
            *user_id == candidate.device.user_id() && root == candidate.device.root_key_public()
        })
        || !metadata
            .pinned_roots
            .iter()
            .any(|(user_id, _)| *user_id == peer_user_id)
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    if let Some(response) = &record.response {
        validate_conversation_provision_response(request, response)?;
    }
    Ok(())
}

fn validate_accepted_conversation_provision(
    record: &ConversationProvisionRecord,
    state: &MlsClientState,
) -> Result<(), DurableMailboxError> {
    if !record.accepted || record.response.is_none() {
        return Err(ConversationError::MetadataMismatch.into());
    }
    validate_conversation_provision_candidate(record, state)
}

fn validate_conversation_provision_response(
    request: &CreateMlsConversationRequest,
    response: &MlsConversationProvisionResponse,
) -> Result<(), DurableMailboxError> {
    if response.conversation_id != request.conversation_id
        || response.group_id != request.group_id
        || response.crypto != "mls_v1"
        || response.epoch != 1
        || response.suite_id != request.suite_id
        || !(0..=MAX_UNIX_TIMESTAMP).contains(&response.provisioned_at_unix)
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    Ok(())
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

fn validate_proposal_ack(
    acknowledgment: &AckE2eeProposalsRequest,
    expected_device_id: DeviceId,
) -> Result<(), KeyStoreError> {
    if acknowledgment.device_id != expected_device_id.to_string()
        || acknowledgment.proposal_ids.is_empty()
        || acknowledgment.proposal_ids.len() > MAX_E2EE_PROPOSAL_ACK_BATCH_SIZE
    {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut ids = HashSet::with_capacity(acknowledgment.proposal_ids.len());
    if acknowledgment
        .proposal_ids
        .iter()
        .all(|value| ProposalId::try_from(value.clone()).is_ok() && ids.insert(value.as_str()))
    {
        Ok(())
    } else {
        Err(KeyStoreError::InvalidValue)
    }
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

pub(crate) fn history_key(group_id: GroupId, message_id: &str) -> Result<StoreKey, KeyStoreError> {
    validate_ulid(message_id)?;
    StoreKey::new(format!("history:{group_id}:{message_id}"))
}

fn message_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:message_ack:{group_id}"))
}

fn commit_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:commit_ack:{group_id}"))
}

fn proposal_ack_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:proposal_ack:{group_id}"))
}

fn outbound_commit_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:outbound_commit:{group_id}"))
}

fn outbound_message_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("mailbox:outbound_message:{group_id}"))
}

fn retention_policy_key(group_id: GroupId) -> Result<StoreKey, KeyStoreError> {
    StoreKey::new(format!("settings:disappearing:{group_id}"))
}

fn load_outbound_commit_record(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
) -> Result<Option<OutboundCommitRecord>, KeyStoreError> {
    let key = outbound_commit_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let record: OutboundCommitRecord =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    if record.version != OUTBOUND_COMMIT_RECORD_VERSION {
        return Err(KeyStoreError::InvalidValue);
    }
    Ok(Some(record))
}

fn load_outbound_message_record(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
) -> Result<Option<OutboundMessageRecord>, KeyStoreError> {
    let key = outbound_message_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let record: OutboundMessageRecord =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    if record.version != OUTBOUND_MESSAGE_RECORD_VERSION
        || record.accepted != record.response.is_some()
        || record.plaintext.is_empty()
        || record.plaintext.len() > MAX_APPLICATION_PLAINTEXT_BYTES
    {
        return Err(KeyStoreError::InvalidValue);
    }
    if let Some(response) = &record.response {
        validate_message_response(response).map_err(|_| KeyStoreError::InvalidValue)?;
    }
    Ok(Some(record))
}

fn validate_outbound_message_record(
    state: &MlsClientState,
    group_id: GroupId,
    record: &OutboundMessageRecord,
) -> Result<(), DurableMailboxError> {
    let request = &record.request;
    if request.sender_device_id != state.device.device_id().to_string()
        || request.suite_id != CiphersuiteId::baseline().as_u16()
        || !matches!(request.message_blob.len(), 512 | 1_024 | 4_096 | 16_384)
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    let conversation = state
        .conversations
        .iter()
        .find(|conversation| conversation.group_id() == group_id)
        .ok_or(DurableMailboxError::ConversationNotFound)?;
    let metadata = conversation.persistence_metadata();
    if metadata.epoch != request.epoch
        || metadata.own_device_id != state.device.device_id()
        || record.generation.checked_add(1) != Some(metadata.outbound_generation)
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    let event = VersionedApplicationEvent::decode(&record.plaintext)?;
    if event.retention_secs != request.retention_secs || event.encode()? != record.plaintext {
        return Err(ConversationError::MetadataMismatch.into());
    }
    Ok(())
}

fn validate_message_response(response: &PostMessageResponse) -> Result<(), DurableMailboxError> {
    validate_ulid(&response.message_id).map_err(DurableMailboxError::KeyStore)?;
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&response.created_at_unix) {
        return Err(ConversationError::MetadataMismatch.into());
    }
    Ok(())
}

fn outbound_stored_message(
    group_id: GroupId,
    sender_user_id: UserId,
    sender_device_id: DeviceId,
    record: &OutboundMessageRecord,
    response: &PostMessageResponse,
) -> Result<StoredMailboxMessage, DurableMailboxError> {
    let expires_at_unix = record
        .request
        .retention_secs
        .map(|duration| {
            response
                .created_at_unix
                .checked_add(
                    i64::try_from(duration.as_u64())
                        .map_err(|_| ConversationError::MetadataMismatch)?,
                )
                .filter(|expires_at| *expires_at <= MAX_UNIX_TIMESTAMP)
                .ok_or(ConversationError::MetadataMismatch)
        })
        .transpose()?;
    Ok(StoredMailboxMessage {
        message_id: response.message_id.clone(),
        group_id,
        created_at_unix: response.created_at_unix,
        expires_at_unix,
        message: DecryptedApplicationMessage {
            sender_user_id,
            sender_device_id,
            generation: record.generation,
            plaintext: record.plaintext.clone(),
        },
    })
}

fn validate_outbound_message_history(
    store: &dyn LocalKeyStore,
    state: &MlsClientState,
    group_id: GroupId,
    record: &OutboundMessageRecord,
    response: &PostMessageResponse,
) -> Result<(), DurableMailboxError> {
    let stored = load_stored_message_at(store, group_id, &response.message_id, 0)?;
    let expected = outbound_stored_message(
        group_id,
        state.device.user_id(),
        state.device.device_id(),
        record,
        response,
    )?;
    if stored != expected {
        return Err(ConversationError::MetadataMismatch.into());
    }
    if let EncryptedChatEvent::SetDisappearingTimer { retention_secs } =
        VersionedApplicationEvent::decode(&record.plaintext)?.event
    {
        if load_disappearing_timer(store, group_id)? != retention_secs {
            return Err(ConversationError::MetadataMismatch.into());
        }
    }
    Ok(())
}

fn validate_outbound_commit_request(
    expected_device_id: DeviceId,
    request: &PostCommitRequest,
) -> Result<(), DurableMailboxError> {
    if request.prior_epoch.checked_add(1) != Some(request.epoch)
        || request.committer_device_id != expected_device_id.to_string()
        || request.commit_blob.is_empty()
        || request.commit_blob.len() > 65_536
        || request.welcome_blob.is_some()
        || request.welcome_device_id.is_some()
        || request
            .group_info_blob
            .as_ref()
            .is_some_and(|blob| blob.is_empty() || blob.len() > 65_536)
    {
        return Err(ConversationError::MetadataMismatch.into());
    }
    let Some(MlsMembershipChange::Remove { leaves }) = &request.membership_change else {
        return Err(ConversationError::UnexpectedMembership.into());
    };
    if leaves.len() != 1
        || UserId::try_from(leaves[0].user_id.clone()).is_err()
        || DeviceId::try_from(leaves[0].device_id.clone()).is_err()
    {
        return Err(ConversationError::UnexpectedMembership.into());
    }
    Ok(())
}

fn rebased_remove_request(
    pending: &PendingGroupCommit,
    membership_change: Option<MlsMembershipChange>,
) -> Result<PostCommitRequest, DurableMailboxError> {
    if pending.welcome_blob.is_some()
        || !matches!(membership_change, Some(MlsMembershipChange::Remove { .. }))
    {
        return Err(ConversationError::UnexpectedMembership.into());
    }
    Ok(PostCommitRequest {
        epoch: pending.epoch,
        prior_epoch: pending.prior_epoch,
        committer_device_id: pending.committer_device_id.to_string(),
        commit_blob: pending.commit_blob.clone(),
        welcome_blob: None,
        welcome_device_id: None,
        group_info_blob: pending.group_info_blob.clone(),
        membership_change,
    })
}

fn validate_winner_membership_routing(
    routing: Option<&MlsMembershipChange>,
    authenticated: Option<&AuthenticatedMembershipChange>,
) -> Result<(), ConversationError> {
    match (routing, authenticated) {
        (None, None) => Ok(()),
        (Some(MlsMembershipChange::Add { leaf }), Some(change))
            if change.kind == AuthenticatedMembershipChangeKind::Added =>
        {
            let user_id = UserId::try_from(leaf.user_id.clone())
                .map_err(|_| ConversationError::MetadataMismatch)?;
            let device_id = DeviceId::try_from(leaf.device_id.clone())
                .map_err(|_| ConversationError::MetadataMismatch)?;
            if change.target_user_id == user_id && change.target_device_ids == [device_id] {
                Ok(())
            } else {
                Err(ConversationError::MetadataMismatch)
            }
        }
        (Some(MlsMembershipChange::Remove { leaves }), Some(change))
            if change.kind == AuthenticatedMembershipChangeKind::Removed && !leaves.is_empty() =>
        {
            let mut routed_devices = HashSet::with_capacity(leaves.len());
            for leaf in leaves {
                let user_id = UserId::try_from(leaf.user_id.clone())
                    .map_err(|_| ConversationError::MetadataMismatch)?;
                let device_id = DeviceId::try_from(leaf.device_id.clone())
                    .map_err(|_| ConversationError::MetadataMismatch)?;
                if user_id != change.target_user_id || !routed_devices.insert(device_id) {
                    return Err(ConversationError::MetadataMismatch);
                }
            }
            let authenticated_devices = change
                .target_device_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>();
            if routed_devices == authenticated_devices {
                Ok(())
            } else {
                Err(ConversationError::MetadataMismatch)
            }
        }
        _ => Err(ConversationError::MetadataMismatch),
    }
}

fn validate_proposal_page(page: &E2eeProposalMailboxResponse) -> Result<(), DurableMailboxError> {
    if page.proposals.len() > MAX_E2EE_PROPOSAL_MAILBOX_PAGE_SIZE {
        return Err(ConversationError::InvalidMailboxPage.into());
    }
    let aggregate = page.proposals.iter().try_fold(0_usize, |total, entry| {
        total.checked_add(entry.proposal_blob.len())
    });
    if aggregate.is_none_or(|total| total > MAX_E2EE_PROPOSAL_MAILBOX_PAGE_BLOB_BYTES) {
        return Err(ConversationError::InvalidMailboxPage.into());
    }
    let mut previous_id: Option<&str> = None;
    let mut ids = HashSet::with_capacity(page.proposals.len());
    for entry in &page.proposals {
        if ProposalId::try_from(entry.proposal_id.clone()).is_err()
            || !ids.insert(entry.proposal_id.as_str())
            || previous_id.is_some_and(|previous| previous >= entry.proposal_id.as_str())
            || entry.epoch == 0
            || entry.proposal_blob.is_empty()
            || entry.proposal_blob.len() > MAX_PROPOSAL_BYTES
            || entry.created_at_unix < 0
            || entry.expires_at_unix <= entry.created_at_unix
            || entry.reconciliation_deadline_unix.is_some_and(|deadline| {
                deadline < entry.created_at_unix || deadline > entry.expires_at_unix
            })
            || matches!(
                (&entry.proposer_device_id, entry.external_sender_index),
                (Some(_), Some(_)) | (None, None)
            )
            || entry
                .proposer_device_id
                .as_ref()
                .is_some_and(|device_id| DeviceId::try_from(device_id.clone()).is_err())
        {
            return Err(ConversationError::InvalidMailboxPage.into());
        }
        previous_id = Some(entry.proposal_id.as_str());
    }
    match (
        page.proposals.last(),
        page.next_after_proposal_id.as_deref(),
    ) {
        (None, None) => Ok(()),
        (Some(last), Some(cursor)) if cursor == last.proposal_id => Ok(()),
        _ => Err(ConversationError::InvalidMailboxPage.into()),
    }
}

/// Load the authenticated per-conversation timer used by the native composer.
///
/// # Errors
/// Rejects corrupt or unknown policy records and propagates store failures.
pub fn load_disappearing_timer(
    store: &dyn LocalKeyStore,
    group_id: GroupId,
) -> Result<Option<E2eeRetentionSeconds>, KeyStoreError> {
    let key = retention_policy_key(group_id)?;
    if !store.exists(&key)? {
        return Ok(None);
    }
    let encoded = store.load(&key)?;
    let policy: RetentionPolicyRecord =
        serde_json::from_slice(&encoded).map_err(|_| KeyStoreError::InvalidValue)?;
    if policy.version != RETENTION_POLICY_VERSION {
        return Err(KeyStoreError::InvalidValue);
    }
    Ok(policy.retention_secs)
}

/// Atomically hard-delete every expired authenticated local-history record.
///
/// # Errors
/// Fails closed on invalid clocks, corrupt history, or encrypted-store errors.
pub fn purge_expired_messages(
    store: &dyn LocalKeyStore,
    now_unix: i64,
) -> Result<usize, KeyStoreError> {
    if !(0..=MAX_UNIX_TIMESTAMP).contains(&now_unix) {
        return Err(KeyStoreError::InvalidValue);
    }
    let mut expired = Vec::new();
    for key in store.list_keys()? {
        let Some((group_id, message_id)) = parse_history_key(&key)? else {
            continue;
        };
        let encoded = store.load(&key)?;
        let message = decode_stored_message(group_id, &message_id, &encoded)?;
        if message
            .expires_at_unix
            .is_some_and(|expires_at| expires_at <= now_unix)
        {
            expired.push(key);
        }
    }
    if expired.is_empty() {
        Ok(0)
    } else {
        store.remove_batch(&expired)
    }
}

pub(crate) fn parse_history_key(
    key: &StoreKey,
) -> Result<Option<(GroupId, String)>, KeyStoreError> {
    let Some(suffix) = key.as_str().strip_prefix("history:") else {
        return Ok(None);
    };
    let (group_id, message_id) = suffix
        .split_once(':')
        .ok_or(KeyStoreError::InvalidIdentifier)?;
    let group_id =
        GroupId::try_from(group_id.to_owned()).map_err(|_| KeyStoreError::InvalidIdentifier)?;
    validate_ulid(message_id)?;
    Ok(Some((group_id, message_id.to_owned())))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
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
    use std::sync::atomic::{AtomicBool, Ordering};

    use filament_core::{
        CiphersuiteId, ConversationCrypto, ConversationId, DeviceId, GroupId, ProposalId, UserId,
    };
    use filament_protocol::{
        E2eeCommitMailboxEntry, E2eeMailboxMessage, E2eeProposalMailboxEntry,
        E2eeProposalMailboxResponse, GroupInfoResponse, MlsMembershipChange, PostCommitResponse,
        PostMessageResponse,
    };

    use super::*;
    use crate::{
        generate_key_package_batch, persist_mls_client_state, ApplicationEventId, ChatMessageBody,
        DeliveryServiceSigner, EncryptedChatEvent, EncryptedMessageId, InMemoryKeyStore, MlsDevice,
        RootIdentityKey, VersionedApplicationEvent, DELIVERY_SERVICE_SEED_BYTES,
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

    #[test]
    fn direct_message_provision_is_exactly_retryable_and_joins_the_peer() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let claimed = ClaimKeyPackageResponse {
            device_id: bob.device_id().to_string(),
            key_package_blob: generate_key_package_batch(&bob, 1).unwrap().remove(0).blob,
            is_last_resort: false,
        };
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[]).unwrap();
        let conversation_id = ConversationId::new();
        let group_id = GroupId::new();
        let runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_direct_message_provision(&store, conversation_id, group_id, bob_pin, &claimed)
            .unwrap();
        assert_eq!(request.welcome_device_id, bob.device_id().to_string());
        assert_eq!(request.committer_device_id, alice.device_id().to_string());
        assert!(runtime.mailbox_routes().unwrap().is_empty());
        drop(runtime);

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted.pending_conversation_provision(&store).unwrap(),
            Some(request.clone())
        );
        let response = MlsConversationProvisionResponse {
            conversation_id: conversation_id.to_string(),
            group_id: group_id.to_string(),
            crypto: String::from("mls_v1"),
            epoch: 1,
            suite_id: CiphersuiteId::baseline().as_u16(),
            provisioned_at_unix: 100,
        };
        restarted
            .confirm_conversation_provision(&store, &request, &response)
            .unwrap();
        assert_eq!(
            restarted.pending_conversation_provision(&store).unwrap(),
            None
        );
        assert_eq!(restarted.mailbox_routes().unwrap().len(), 1);

        let mut bob_group =
            MlsConversation::join_from_welcome(group_id, &bob, alice_pin, &request.welcome_blob)
                .unwrap();
        let event = restarted
            .state
            .as_mut()
            .unwrap()
            .conversations
            .first_mut()
            .unwrap()
            .encrypt_application_message(&alice, b"provisioned")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &event)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"provisioned"
        );
    }

    #[test]
    fn direct_message_provision_rejects_routing_and_response_substitution() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let key_package = generate_key_package_batch(&bob, 2).unwrap();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let wrong_route = ClaimKeyPackageResponse {
            device_id: DeviceId::new().to_string(),
            key_package_blob: key_package[0].blob.clone(),
            is_last_resort: false,
        };
        assert_eq!(
            runtime.prepare_direct_message_provision(
                &store,
                ConversationId::new(),
                GroupId::new(),
                bob_pin,
                &wrong_route,
            ),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert!(!store
            .exists(&StoreKey::pending_conversation_provision())
            .unwrap());

        let conversation_id = ConversationId::new();
        let group_id = GroupId::new();
        let request = runtime
            .prepare_direct_message_provision(
                &store,
                conversation_id,
                group_id,
                bob_pin,
                &ClaimKeyPackageResponse {
                    device_id: bob.device_id().to_string(),
                    key_package_blob: key_package[1].blob.clone(),
                    is_last_resort: false,
                },
            )
            .unwrap();
        let substituted = MlsConversationProvisionResponse {
            conversation_id: ConversationId::new().to_string(),
            group_id: group_id.to_string(),
            crypto: String::from("mls_v1"),
            epoch: 1,
            suite_id: CiphersuiteId::baseline().as_u16(),
            provisioned_at_unix: 100,
        };
        assert_eq!(
            runtime.confirm_conversation_provision(&store, &request, &substituted),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert_eq!(
            runtime.pending_conversation_provision(&store).unwrap(),
            Some(request)
        );
        assert!(runtime.mailbox_routes().unwrap().is_empty());
    }

    #[test]
    fn direct_message_provision_cannot_roll_back_newer_group_progress() {
        let JoinedFixture {
            alice, alice_group, ..
        } = joined_fixture();
        let charlie_root = RootIdentityKey::generate();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let claimed = ClaimKeyPackageResponse {
            device_id: charlie.device_id().to_string(),
            key_package_blob: generate_key_package_batch(&charlie, 1)
                .unwrap()
                .remove(0)
                .blob,
            is_last_resort: false,
        };
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[&alice_group]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_direct_message_provision(
                &store,
                ConversationId::new(),
                GroupId::new(),
                charlie_pin,
                &claimed,
            )
            .unwrap();

        let state = runtime.state.as_mut().unwrap();
        state.conversations[0]
            .create_self_update(&state.device)
            .unwrap();
        state.conversations[0]
            .accept_pending_commit(&state.device)
            .unwrap();
        let conversations = state.conversations.iter().collect::<Vec<_>>();
        persist_mls_client_state(&store, &state.device, &conversations).unwrap();
        let response = MlsConversationProvisionResponse {
            conversation_id: request.conversation_id.clone(),
            group_id: request.group_id.clone(),
            crypto: String::from("mls_v1"),
            epoch: 1,
            suite_id: request.suite_id,
            provisioned_at_unix: 100,
        };

        assert_eq!(
            runtime.confirm_conversation_provision(&store, &request, &response),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert_eq!(
            runtime.pending_conversation_provision(&store),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        let restored = crate::load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), 2);
    }

    #[test]
    fn outbound_message_checkpoint_retry_and_local_history_are_one_durable_flow() {
        let JoinedFixture {
            alice,
            alice_group,
            bob,
            mut bob_group,
            group_id,
        } = joined_fixture();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[&alice_group]).unwrap();
        let event = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from(String::from("durable hello")).unwrap(),
                reply: None,
            },
        };

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_outbound_message(&store, group_id, &event)
            .unwrap();
        assert_eq!(request.retention_secs, None);
        assert_eq!(request.sender_device_id, alice.device_id().to_string());
        drop(runtime);

        let restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted
                .pending_outbound_message(&store, group_id)
                .unwrap(),
            Some(request.clone())
        );
        let decrypted = bob_group
            .decrypt_application_message(
                &bob,
                &crate::EncryptedApplicationMessage {
                    crypto: ConversationCrypto::MlsV1,
                    group_id,
                    epoch: request.epoch,
                    suite: CiphersuiteId::try_from(request.suite_id).unwrap(),
                    sender_device_id: DeviceId::try_from(request.sender_device_id.clone()).unwrap(),
                    message_blob: request.message_blob.clone(),
                },
            )
            .unwrap();
        assert_eq!(decrypted.authenticated_message.generation, 0);
        assert_eq!(
            VersionedApplicationEvent::decode(&decrypted.authenticated_message.plaintext).unwrap(),
            event
        );

        let response = PostMessageResponse {
            message_id: Ulid::new().to_string(),
            created_at_unix: 100,
        };
        restarted
            .confirm_outbound_message(&store, group_id, &request, &response)
            .unwrap();
        assert_eq!(
            restarted
                .pending_outbound_message(&store, group_id)
                .unwrap(),
            None
        );
        let stored = load_stored_message_at(&store, group_id, &response.message_id, 100).unwrap();
        assert_eq!(stored.message.sender_user_id, alice.user_id());
        assert_eq!(stored.message.sender_device_id, alice.device_id());
        assert_eq!(stored.message.generation, 0);
        assert_eq!(
            VersionedApplicationEvent::decode(&stored.message.plaintext).unwrap(),
            event
        );
    }

    #[test]
    fn outbound_message_rejects_substitution_and_updates_authenticated_timer() {
        let JoinedFixture {
            alice,
            alice_group,
            group_id,
            ..
        } = joined_fixture();
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[&alice_group]).unwrap();
        let timer = E2eeRetentionSeconds::new(60).unwrap();
        let event = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::SetDisappearingTimer {
                retention_secs: Some(timer),
            },
        };
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_outbound_message(&store, group_id, &event)
            .unwrap();
        let substituted = PostMessageResponse {
            message_id: String::from("not-a-ulid"),
            created_at_unix: 100,
        };
        assert!(matches!(
            runtime.confirm_outbound_message(&store, group_id, &request, &substituted),
            Err(DurableMailboxError::KeyStore(
                KeyStoreError::InvalidIdentifier
            ))
        ));
        assert_eq!(
            runtime.pending_outbound_message(&store, group_id).unwrap(),
            Some(request.clone())
        );

        let response = PostMessageResponse {
            message_id: Ulid::new().to_string(),
            created_at_unix: 100,
        };
        runtime
            .confirm_outbound_message(&store, group_id, &request, &response)
            .unwrap();
        assert_eq!(
            load_disappearing_timer(&store, group_id).unwrap(),
            Some(timer)
        );

        let wrong_policy_event = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from(String::from("must inherit timer")).unwrap(),
                reply: None,
            },
        };
        assert!(matches!(
            runtime.prepare_outbound_message(&store, group_id, &wrong_policy_event),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        ));
    }

    struct RejectFirstRemoveStore {
        inner: InMemoryKeyStore,
        reject_remove: AtomicBool,
    }

    impl LocalKeyStore for RejectFirstRemoveStore {
        fn store(&self, key: StoreKey, value: Vec<u8>) -> Result<(), KeyStoreError> {
            self.inner.store(key, value)
        }

        fn store_batch(&self, entries: Vec<(StoreKey, Vec<u8>)>) -> Result<(), KeyStoreError> {
            self.inner.store_batch(entries)
        }

        fn store_batch_if_absent_or_equal(
            &self,
            entries: Vec<(StoreKey, Vec<u8>)>,
        ) -> Result<usize, KeyStoreError> {
            self.inner.store_batch_if_absent_or_equal(entries)
        }

        fn load(&self, key: &StoreKey) -> Result<Zeroizing<Vec<u8>>, KeyStoreError> {
            self.inner.load(key)
        }

        fn remove(&self, key: &StoreKey) -> Result<(), KeyStoreError> {
            self.inner.remove(key)
        }

        fn remove_batch(&self, keys: &[StoreKey]) -> Result<usize, KeyStoreError> {
            if self.reject_remove.swap(false, Ordering::SeqCst) {
                Err(KeyStoreError::BackendError)
            } else {
                self.inner.remove_batch(keys)
            }
        }

        fn exists(&self, key: &StoreKey) -> Result<bool, KeyStoreError> {
            self.inner.exists(key)
        }

        fn list_keys(&self) -> Result<Vec<StoreKey>, KeyStoreError> {
            self.inner.list_keys()
        }
    }

    #[test]
    fn accepted_conversation_marker_closes_cleanup_crash_window() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let claimed = ClaimKeyPackageResponse {
            device_id: bob.device_id().to_string(),
            key_package_blob: generate_key_package_batch(&bob, 1).unwrap().remove(0).blob,
            is_last_resort: false,
        };
        let store = RejectFirstRemoveStore {
            inner: InMemoryKeyStore::new(),
            reject_remove: AtomicBool::new(true),
        };
        persist_mls_client_state(&store.inner, &alice, &[]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_direct_message_provision(
                &store,
                ConversationId::new(),
                GroupId::new(),
                bob_pin,
                &claimed,
            )
            .unwrap();
        let response = MlsConversationProvisionResponse {
            conversation_id: request.conversation_id.clone(),
            group_id: request.group_id.clone(),
            crypto: String::from("mls_v1"),
            epoch: 1,
            suite_id: request.suite_id,
            provisioned_at_unix: 100,
        };
        assert!(matches!(
            runtime.confirm_conversation_provision(&store, &request, &response),
            Err(DurableMailboxError::KeyStore(KeyStoreError::BackendError))
        ));
        assert!(!runtime.is_ready());

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted.pending_conversation_provision(&store).unwrap(),
            None
        );
        assert_eq!(restarted.mailbox_routes().unwrap().len(), 1);
        assert!(!store
            .exists(&StoreKey::pending_conversation_provision())
            .unwrap());
        assert!(!store
            .exists(&StoreKey::pending_conversation_checkpoint())
            .unwrap());
    }

    #[test]
    fn accepted_outbound_message_marker_closes_history_cleanup_crash_window() {
        let JoinedFixture {
            alice,
            alice_group,
            group_id,
            ..
        } = joined_fixture();
        let store = RejectFirstRemoveStore {
            inner: InMemoryKeyStore::new(),
            reject_remove: AtomicBool::new(true),
        };
        persist_mls_client_state(&store.inner, &alice, &[&alice_group]).unwrap();
        let event = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from(String::from("accepted marker")).unwrap(),
                reply: None,
            },
        };
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .prepare_outbound_message(&store, group_id, &event)
            .unwrap();
        let response = PostMessageResponse {
            message_id: Ulid::new().to_string(),
            created_at_unix: 100,
        };
        assert!(matches!(
            runtime.confirm_outbound_message(&store, group_id, &request, &response),
            Err(DurableMailboxError::KeyStore(KeyStoreError::BackendError))
        ));
        assert_eq!(
            VersionedApplicationEvent::decode(
                &load_stored_message_at(&store, group_id, &response.message_id, 100)
                    .unwrap()
                    .message
                    .plaintext
            )
            .unwrap(),
            event
        );

        let restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted
                .pending_outbound_message(&store, group_id)
                .unwrap(),
            None
        );
        assert!(!store
            .exists(&outbound_message_key(group_id).unwrap())
            .unwrap());
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

    struct ProposalFixture {
        alice: MlsDevice,
        bob: MlsDevice,
        bob_group: MlsConversation,
        charlie: MlsDevice,
        group_id: GroupId,
        page: E2eeProposalMailboxResponse,
        store: InMemoryKeyStore,
    }

    fn external_remove_proposal_fixture() -> ProposalFixture {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let delivery =
            DeliveryServiceSigner::from_seed([0x31; DELIVERY_SERVICE_SEED_BYTES]).unwrap();
        let group_id = GroupId::new();
        let (mut group, initial) = MlsConversation::create_group_with_delivery_service(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
            delivery.identity(),
        )
        .unwrap();
        group.accept_pending_commit(&alice).unwrap();
        let bob_group = MlsConversation::join_group_from_welcome_with_delivery_service(
            group_id,
            &bob,
            &[alice_pin, charlie_pin],
            initial.welcome_blob.as_deref().unwrap(),
            delivery.identity(),
        )
        .unwrap();
        let proposal = delivery.sign_remove(group_id, group.epoch(), 2).unwrap();
        let proposal_id = ProposalId::new().to_string();
        let page = E2eeProposalMailboxResponse {
            proposals: vec![E2eeProposalMailboxEntry {
                proposal_id: proposal_id.clone(),
                epoch: proposal.epoch,
                proposer_device_id: None,
                external_sender_index: Some(0),
                proposal_blob: proposal.proposal_blob,
                created_at_unix: 100,
                expires_at_unix: 200,
                reconciliation_deadline_unix: Some(150),
            }],
            next_after_proposal_id: Some(proposal_id.clone()),
        };
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &alice, &[&group]).unwrap();
        ProposalFixture {
            alice,
            bob,
            bob_group,
            charlie,
            group_id,
            page,
            store,
        }
    }

    #[test]
    fn external_remove_proposal_commit_and_ack_outboxes_survive_response_loss() {
        let ProposalFixture {
            alice,
            charlie,
            group_id,
            page,
            store,
            ..
        } = external_remove_proposal_fixture();
        let proposal_id = page.proposals[0].proposal_id.clone();
        let mut runtime = DurableMlsClient::load(&store).unwrap();

        let batch = runtime
            .process_proposal_mailbox(&store, group_id, page)
            .unwrap();
        assert_eq!(batch.processed_proposal_ids, vec![proposal_id.clone()]);
        assert!(!batch.awaiting_peer_commit);
        let acknowledgment = batch.acknowledgment.unwrap();
        assert_eq!(acknowledgment.proposal_ids, vec![proposal_id]);
        let request = batch.outbound_commit.unwrap();
        assert_eq!(request.prior_epoch, 1);
        assert_eq!(request.epoch, 2);
        assert!(request.welcome_blob.is_none());
        assert!(request.welcome_device_id.is_none());
        let Some(MlsMembershipChange::Remove { leaves }) = &request.membership_change else {
            panic!("policy proposal must produce one authenticated Remove routing delta");
        };
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].leaf_index, 2);
        assert_eq!(leaves[0].user_id, charlie.user_id().to_string());
        assert_eq!(leaves[0].device_id, charlie.device_id().to_string());

        let durable_ack = pending_proposal_acknowledgment(&store, group_id, alice.device_id())
            .unwrap()
            .unwrap();
        assert_eq!(durable_ack, acknowledgment);

        // An uncertain network response leaves the exact request retryable.
        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted.pending_outbound_commit(&store, group_id).unwrap(),
            Some(request.clone())
        );
        assert_eq!(
            restarted.pending_outbound_commit(&store, group_id).unwrap(),
            Some(request.clone())
        );

        let mut substituted = request.clone();
        substituted.commit_blob.push(0);
        assert_eq!(
            restarted.confirm_outbound_commit(
                &store,
                group_id,
                &substituted,
                &PostCommitResponse {
                    accepted: true,
                    epoch: request.epoch,
                },
            ),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        );
        assert!(restarted.is_ready());

        restarted
            .confirm_outbound_commit(
                &store,
                group_id,
                &request,
                &PostCommitResponse {
                    accepted: true,
                    epoch: request.epoch,
                },
            )
            .unwrap();
        assert_eq!(
            restarted.pending_outbound_commit(&store, group_id).unwrap(),
            None
        );
        confirm_proposal_acknowledgment(&store, group_id, &acknowledgment).unwrap();

        let restored = crate::load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), request.epoch);
        assert!(!restored.conversations[0]
            .has_verified_member_device(charlie.device_id())
            .unwrap());
    }

    #[test]
    fn accepted_outbound_marker_is_reconciled_only_against_the_merged_epoch() {
        let ProposalFixture {
            alice,
            group_id,
            page,
            store,
            ..
        } = external_remove_proposal_fixture();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let request = runtime
            .process_proposal_mailbox(&store, group_id, page)
            .unwrap()
            .outbound_commit
            .unwrap();

        // Model a crash after the accepted checkpoint transaction but before
        // the best-effort accepted-marker cleanup.
        let mut accepted_state = crate::load_mls_client_state(&store).unwrap();
        accepted_state.conversations[0]
            .accept_pending_commit(&accepted_state.device)
            .unwrap();
        store
            .store_batch(vec![
                (
                    StoreKey::mls_client_state(),
                    encode_state(&accepted_state).unwrap(),
                ),
                (
                    outbound_commit_key(group_id).unwrap(),
                    encode_json(&OutboundCommitRecord {
                        version: OUTBOUND_COMMIT_RECORD_VERSION,
                        accepted: true,
                        invalidated: false,
                        request,
                    })
                    .unwrap(),
                ),
            ])
            .unwrap();

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted.pending_outbound_commit(&store, group_id).unwrap(),
            None
        );
        assert!(!store
            .exists(&outbound_commit_key(group_id).unwrap())
            .unwrap());
        assert_eq!(
            crate::load_mls_client_state(&store)
                .unwrap()
                .device
                .device_id(),
            alice.device_id()
        );
    }

    #[test]
    fn proposal_commit_rebases_durably_on_authenticated_epoch_winner() {
        let ProposalFixture {
            alice,
            bob,
            mut bob_group,
            charlie,
            group_id,
            page,
            store,
        } = external_remove_proposal_fixture();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let rejected = runtime
            .process_proposal_mailbox(&store, group_id, page)
            .unwrap()
            .outbound_commit
            .unwrap();

        let winner = bob_group.create_self_update(&bob).unwrap();
        assert_eq!(winner.epoch, rejected.epoch);
        bob_group.accept_pending_commit(&bob).unwrap();
        let winner_page = E2eeCommitMailboxResponse {
            commits: vec![E2eeCommitMailboxEntry {
                epoch: winner.epoch,
                prior_epoch: winner.prior_epoch,
                committer_device_id: winner.committer_device_id.to_string(),
                commit_blob: winner.commit_blob,
                welcome_blob: None,
                membership_change: None,
                created_at_unix: 101,
                expires_at_unix: 201,
            }],
            next_after_epoch: Some(winner.epoch),
        };
        let rebased = runtime
            .rebase_outbound_commit(&store, group_id, winner_page)
            .unwrap();
        assert_eq!(rebased.acknowledgment.epochs, vec![winner.epoch]);
        assert!(!rebased.already_satisfied);
        assert!(!rebased.invalidated);
        let replacement = rebased.outbound_commit.unwrap();
        assert_eq!(replacement.prior_epoch, winner.epoch);
        assert_eq!(replacement.epoch, winner.epoch + 1);
        assert_ne!(replacement.commit_blob, rejected.commit_blob);
        let Some(MlsMembershipChange::Remove { leaves }) = &replacement.membership_change else {
            panic!("rebased policy intent must remain an exact Remove");
        };
        assert_eq!(leaves[0].device_id, charlie.device_id().to_string());

        assert_eq!(
            pending_commit_acknowledgment(&store, group_id, alice.device_id())
                .unwrap()
                .unwrap(),
            rebased.acknowledgment
        );
        confirm_commit_acknowledgment(&store, group_id, &rebased.acknowledgment).unwrap();

        let mut restarted = DurableMlsClient::load(&store).unwrap();
        assert_eq!(
            restarted.pending_outbound_commit(&store, group_id).unwrap(),
            Some(replacement.clone())
        );
        restarted
            .confirm_outbound_commit(
                &store,
                group_id,
                &replacement,
                &PostCommitResponse {
                    accepted: true,
                    epoch: replacement.epoch,
                },
            )
            .unwrap();
        let proposal_ack = pending_proposal_acknowledgment(&store, group_id, alice.device_id())
            .unwrap()
            .unwrap();
        confirm_proposal_acknowledgment(&store, group_id, &proposal_ack).unwrap();

        let restored = crate::load_mls_client_state(&store).unwrap();
        assert_eq!(restored.conversations[0].epoch(), replacement.epoch);
        assert!(!restored.conversations[0]
            .has_verified_member_device(charlie.device_id())
            .unwrap());
    }

    #[test]
    fn epoch_winner_routing_mismatch_shuts_down_until_checkpoint_reload() {
        let ProposalFixture {
            alice,
            bob,
            mut bob_group,
            group_id,
            page,
            store,
            ..
        } = external_remove_proposal_fixture();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let rejected = runtime
            .process_proposal_mailbox(&store, group_id, page)
            .unwrap()
            .outbound_commit
            .unwrap();
        let winner = bob_group.create_self_update(&bob).unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();
        let hostile_page = E2eeCommitMailboxResponse {
            commits: vec![E2eeCommitMailboxEntry {
                epoch: winner.epoch,
                prior_epoch: winner.prior_epoch,
                committer_device_id: winner.committer_device_id.to_string(),
                commit_blob: winner.commit_blob,
                welcome_blob: None,
                membership_change: rejected.membership_change.clone(),
                created_at_unix: 101,
                expires_at_unix: 201,
            }],
            next_after_epoch: Some(winner.epoch),
        };

        assert!(matches!(
            runtime.rebase_outbound_commit(&store, group_id, hostile_page),
            Err(DurableMailboxError::Conversation(
                ConversationError::MetadataMismatch
            ))
        ));
        assert!(!runtime.is_ready());
        runtime.reload(&store).unwrap();
        assert_eq!(
            runtime.pending_outbound_commit(&store, group_id).unwrap(),
            Some(rejected)
        );
        assert_eq!(
            crate::load_mls_client_state(&store).unwrap().conversations[0].epoch(),
            1
        );
        assert!(
            pending_commit_acknowledgment(&store, group_id, alice.device_id())
                .unwrap()
                .is_none()
        );
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
    fn authenticated_timer_is_durable_and_expired_history_is_hard_deleted() {
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
        let timer = E2eeRetentionSeconds::new(60).unwrap();
        let policy = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::SetDisappearingTimer {
                retention_secs: Some(timer),
            },
        };
        let disappearing = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: Some(timer),
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from("vanishes locally".to_owned()).unwrap(),
                reply: None,
            },
        };
        let policy_id = Ulid::new().to_string();
        let message_id = Ulid::new().to_string();
        let policy_ciphertext = alice_group.encrypt_chat_event(&alice, &policy).unwrap();
        let message_ciphertext = alice_group
            .encrypt_chat_event(&alice, &disappearing)
            .unwrap();
        let page = E2eeMailboxResponse {
            messages: vec![
                message_entry(policy_id.clone(), policy_ciphertext),
                message_entry(message_id.clone(), message_ciphertext),
            ],
            next_after_message_id: Some(message_id.clone()),
        };

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let batch = runtime
            .process_message_mailbox_at(&store, group_id, page, 20)
            .unwrap();
        assert_eq!(batch.ready_messages.len(), 2);
        confirm_message_acknowledgment(&store, group_id, batch.acknowledgment.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            load_disappearing_timer(&store, group_id).unwrap(),
            Some(timer)
        );
        assert_eq!(
            load_stored_message_at(&store, group_id, &message_id, 69)
                .unwrap()
                .expires_at_unix,
            Some(70)
        );
        assert_eq!(
            load_stored_message_at(&store, group_id, &message_id, 70),
            Err(KeyStoreError::NotFound)
        );
        assert_eq!(
            load_stored_message_at(&store, group_id, &policy_id, i64::MAX)
                .unwrap()
                .expires_at_unix,
            None
        );
    }

    #[test]
    fn already_expired_authenticated_message_is_acked_without_plaintext_release() {
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
        let event = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: Some(E2eeRetentionSeconds::new(60).unwrap()),
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from("must never be exposed".to_owned()).unwrap(),
                reply: None,
            },
        };
        let mut entry = message_entry(
            message_id.clone(),
            alice_group.encrypt_chat_event(&alice, &event).unwrap(),
        );
        entry.created_at_unix = 30;
        entry.expires_at_unix = 90;

        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let batch = runtime
            .process_message_mailbox_at(
                &store,
                group_id,
                E2eeMailboxResponse {
                    messages: vec![entry],
                    next_after_message_id: Some(message_id.clone()),
                },
                90,
            )
            .unwrap();
        assert!(batch.ready_messages.is_empty());
        assert!(batch.acknowledgment.is_some());
        assert_eq!(
            load_stored_message_at(&store, group_id, &message_id, 90),
            Err(KeyStoreError::NotFound)
        );
    }

    #[test]
    fn buffered_generation_is_not_released_after_its_authenticated_expiry() {
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
        let retained = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: None,
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from("fills the gap".to_owned()).unwrap(),
                reply: None,
            },
        };
        let disappearing = VersionedApplicationEvent {
            event_id: ApplicationEventId::new(),
            retention_secs: Some(E2eeRetentionSeconds::new(60).unwrap()),
            event: EncryptedChatEvent::Message {
                message_id: EncryptedMessageId::new(),
                body: ChatMessageBody::try_from("expires while buffered".to_owned()).unwrap(),
                reply: None,
            },
        };
        let retained_ciphertext = alice_group.encrypt_chat_event(&alice, &retained).unwrap();
        let disappearing_ciphertext = alice_group
            .encrypt_chat_event(&alice, &disappearing)
            .unwrap();
        let disappearing_id = Ulid::new().to_string();
        let retained_id = Ulid::new().to_string();
        let mut runtime = DurableMlsClient::load(&store).unwrap();

        let buffered = runtime
            .process_message_mailbox_at(
                &store,
                group_id,
                E2eeMailboxResponse {
                    messages: vec![message_entry(
                        disappearing_id.clone(),
                        disappearing_ciphertext,
                    )],
                    next_after_message_id: Some(disappearing_id),
                },
                20,
            )
            .unwrap();
        assert!(buffered.ready_messages.is_empty());
        confirm_message_acknowledgment(&store, group_id, buffered.acknowledgment.as_ref().unwrap())
            .unwrap();

        let released = runtime
            .process_message_mailbox_at(
                &store,
                group_id,
                E2eeMailboxResponse {
                    messages: vec![message_entry(retained_id.clone(), retained_ciphertext)],
                    next_after_message_id: Some(retained_id),
                },
                80,
            )
            .unwrap();
        assert_eq!(released.ready_messages.len(), 1);
        assert_eq!(
            VersionedApplicationEvent::decode(&released.ready_messages[0].plaintext)
                .unwrap()
                .event,
            retained.event
        );
    }

    #[test]
    fn expiry_sweep_is_bounded_atomic_and_keeps_retained_history() {
        let store = InMemoryKeyStore::new();
        let group_id = GroupId::new();
        let sender_user_id = UserId::new();
        let retained_id = Ulid::new().to_string();
        let expired_id = Ulid::new().to_string();
        for (message_id, expires_at_unix) in
            [(retained_id.clone(), None), (expired_id.clone(), Some(20))]
        {
            let entry = history_storage_entry(&StoredMailboxMessage {
                message_id,
                group_id,
                created_at_unix: 10,
                expires_at_unix,
                message: DecryptedApplicationMessage {
                    sender_user_id,
                    sender_device_id: DeviceId::new(),
                    generation: 0,
                    plaintext: b"authenticated history".to_vec(),
                },
            })
            .unwrap();
            store.store(entry.0, entry.1).unwrap();
        }
        assert_eq!(purge_expired_messages(&store, 20).unwrap(), 1);
        assert_eq!(
            load_stored_message_at(&store, group_id, &expired_id, 20),
            Err(KeyStoreError::NotFound)
        );
        assert!(load_stored_message_at(&store, group_id, &retained_id, 20).is_ok());
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
            membership_change: None,
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
    #[allow(clippy::too_many_lines)]
    fn group_external_commit_recovery_preserves_exact_pinned_audience() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let group_id = GroupId::new();
        let (mut alice_group, initial) = MlsConversation::create_group(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = MlsConversation::join_group_from_welcome(
            group_id,
            &bob,
            &[alice_pin, charlie_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let mut charlie_group = MlsConversation::join_group_from_welcome(
            group_id,
            &charlie,
            &[alice_pin, bob_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        // Bob misses this accepted epoch and must recover from its signed
        // GroupInfo without letting the Delivery Service alter local pins.
        let update = alice_group.create_self_update(&alice).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        charlie_group
            .process_incoming_commit(
                &charlie,
                &crate::EncryptedGroupCommit {
                    group_id,
                    prior_epoch: update.prior_epoch,
                    epoch: update.epoch,
                    committer_device_id: update.committer_device_id,
                    commit_blob: update.commit_blob.clone(),
                },
            )
            .unwrap();
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
        assert_eq!(
            runtime
                .prepare_group_external_commit_recovery(&store, &[alice_pin], &recovery_info)
                .unwrap_err(),
            DurableMailboxError::Conversation(ConversationError::MetadataMismatch)
        );
        let forged_charlie_pin = PinnedUserIdentity::new(
            charlie.user_id(),
            RootIdentityKey::generate().public_key_bytes(),
        );
        assert_eq!(
            runtime
                .prepare_group_external_commit_recovery(
                    &store,
                    &[alice_pin, forged_charlie_pin],
                    &recovery_info
                )
                .unwrap_err(),
            DurableMailboxError::Conversation(ConversationError::MetadataMismatch)
        );

        let recovery = runtime
            .prepare_group_external_commit_recovery(
                &store,
                &[alice_pin, charlie_pin],
                &recovery_info,
            )
            .unwrap();
        let pending = recovery.pending_commit();
        assert_eq!(pending.prior_epoch, update.epoch);
        assert_eq!(pending.epoch, update.epoch + 1);
        let recovery_commit = crate::EncryptedGroupCommit {
            group_id,
            prior_epoch: pending.prior_epoch,
            epoch: pending.epoch,
            committer_device_id: pending.committer_device_id,
            commit_blob: pending.commit_blob.clone(),
        };
        assert!(alice_group
            .persistence_metadata()
            .pinned_roots
            .contains(&(bob.user_id(), *bob.root_key_public())));
        assert!(alice_group
            .persistence_metadata()
            .pinned_roots
            .contains(&(charlie.user_id(), *charlie.root_key_public())));
        alice_group
            .process_incoming_commit(&alice, &recovery_commit)
            .unwrap();
        charlie_group
            .process_incoming_commit(&charlie, &recovery_commit)
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
        let state = restarted.state.as_mut().unwrap();
        let recovered = state
            .conversations
            .iter_mut()
            .find(|conversation| conversation.group_id() == group_id)
            .unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"group recovered after desync")
            .unwrap();
        assert_eq!(
            recovered
                .decrypt_application_message(&state.device, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"group recovered after desync"
        );
        let reply = recovered
            .encrypt_application_message(&state.device, b"group recovery reply")
            .unwrap();
        assert_eq!(
            charlie_group
                .decrypt_application_message(&charlie, &reply)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"group recovery reply"
        );
    }

    #[test]
    fn group_mailbox_route_and_commit_prefix_are_derived_from_the_durable_checkpoint() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let charlie_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let charlie = MlsDevice::generate(UserId::new(), DeviceId::new(), &charlie_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let charlie_pin = PinnedUserIdentity::new(charlie.user_id(), *charlie.root_key_public());
        let bob_package = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let charlie_package = generate_key_package_batch(&charlie, 1)
            .unwrap()
            .remove(0)
            .blob;
        let group_id = GroupId::new();
        let (mut alice_group, initial) = MlsConversation::create_group(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let bob_group = MlsConversation::join_group_from_welcome(
            group_id,
            &bob,
            &[alice_pin, charlie_pin],
            initial.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        let update = alice_group.create_self_update(&alice).unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let update_epoch = update.epoch;
        let page = E2eeCommitMailboxResponse {
            commits: vec![E2eeCommitMailboxEntry {
                epoch: update.epoch,
                prior_epoch: update.prior_epoch,
                committer_device_id: update.committer_device_id.to_string(),
                commit_blob: update.commit_blob,
                welcome_blob: None,
                membership_change: None,
                created_at_unix: 10,
                expires_at_unix: 20,
            }],
            next_after_epoch: Some(update_epoch),
        };
        let store = InMemoryKeyStore::new();
        persist_mls_client_state(&store, &bob, &[&bob_group]).unwrap();
        let mut runtime = DurableMlsClient::load(&store).unwrap();
        let routes = runtime.mailbox_routes().unwrap();
        let mut expected_participants = vec![alice_pin, charlie_pin];
        expected_participants.sort_by_key(|pin| pin.user_id.to_string());
        assert_eq!(
            routes,
            vec![MailboxConversationRoute {
                group_id,
                audience: ConversationAudience::GroupDm,
                participants: expected_participants,
            }]
        );

        let batch = runtime
            .process_group_commit_mailbox(&store, group_id, &routes[0].participants, page)
            .unwrap();
        assert_eq!(batch.processed_epochs, vec![update_epoch]);
        assert_eq!(batch.acknowledgment.unwrap().epochs, vec![update_epoch]);
        assert_eq!(
            crate::load_mls_client_state(&store).unwrap().conversations[0].epoch(),
            update_epoch
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
