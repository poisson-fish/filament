//! Fail-closed client processing for ordered MLS commit mailboxes.
//!
//! Commit pages are attacker-controlled and stateful: unlike application
//! messages, a rejected commit prevents every later epoch from being safely
//! processed. This module therefore validates the complete page before
//! touching MLS state and processes only a successful epoch prefix.

use std::collections::HashSet;

use filament_core::{DeviceId, GroupId, UserId};
use filament_protocol::{
    AckE2eeCommitsRequest, E2eeCommitMailboxEntry, E2eeCommitMailboxResponse, MlsMembershipChange,
    MAX_COMMIT_BYTES, MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE,
    MAX_WELCOME_BYTES,
};

use crate::conversation::validate_commit_envelope;
use crate::{
    AuthenticatedMembershipChange, ConversationError, EncryptedGroupCommit, MlsConversation,
    MlsDevice, PinnedUserIdentity,
};

/// One commit that blocked processing of its epoch and every later epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedMailboxCommit {
    /// Zero-based position in the bounded mailbox page.
    pub entry_index: usize,
    /// Epoch that could not be authenticated and merged.
    pub epoch: u64,
    /// Fail-closed validation or MLS processing error.
    pub error: ConversationError,
}

/// State and acknowledgment material after processing a commit page.
pub struct CommitMailboxBatch {
    /// Epochs newly established by this call, excluding already-durable replay epochs.
    pub processed_epochs: Vec<u64>,
    /// Send only after `conversation` is durably persisted.
    pub pending_acknowledgment: Option<AckE2eeCommitsRequest>,
    /// First failed epoch. Later entries are deliberately left unprocessed.
    pub rejected_commit: Option<RejectedMailboxCommit>,
    /// UI rows derived only from successfully authenticated MLS commits.
    pub membership_changes: Vec<AuthenticatedMembershipChange>,
}

/// Join from a recipient-bound Welcome or advance an existing conversation.
///
/// Acknowledgments cover only epochs represented by the updated conversation
/// state, and must be sent only after that state is durably persisted.
///
/// # Errors
/// Returns [`ConversationError::InvalidMailboxPage`] when count, aggregate
/// bytes, cursors, epoch ordering, timestamps, identifiers, or blob bounds are
/// invalid. No MLS state is touched in that case.
pub fn process_commit_mailbox(
    conversation: &mut Option<MlsConversation>,
    device: &MlsDevice,
    group_id: GroupId,
    peer: PinnedUserIdentity,
    page: E2eeCommitMailboxResponse,
) -> Result<CommitMailboxBatch, ConversationError> {
    validate_page(&page)?;

    let mut processed_epochs = Vec::new();
    let mut acknowledged_epochs = Vec::new();
    let mut rejected_commit = None;

    for (entry_index, entry) in page.commits.into_iter().enumerate() {
        let epoch = entry.epoch;
        let result = process_entry(conversation, device, group_id, peer, entry);
        match result {
            Ok(newly_processed) => {
                if newly_processed {
                    processed_epochs.push(epoch);
                }
                acknowledged_epochs.push(epoch);
            }
            Err(error) => {
                rejected_commit = Some(RejectedMailboxCommit {
                    entry_index,
                    epoch,
                    error,
                });
                break;
            }
        }
    }

    let pending_acknowledgment = (!acknowledged_epochs.is_empty()).then(|| AckE2eeCommitsRequest {
        device_id: device.device_id().to_string(),
        epochs: acknowledged_epochs,
    });
    Ok(CommitMailboxBatch {
        processed_epochs,
        pending_acknowledgment,
        rejected_commit,
        membership_changes: Vec::new(),
    })
}

/// Process a group-DM commit page against an exact caller-pinned participant set.
///
/// # Errors
/// Rejects malformed pages, unpinned Adds, routing-hint mismatches, invalid
/// Welcomes, non-contiguous epochs, and any commit that fails MLS authentication.
pub fn process_group_commit_mailbox(
    conversation: &mut Option<MlsConversation>,
    device: &MlsDevice,
    group_id: GroupId,
    participants: &[PinnedUserIdentity],
    page: E2eeCommitMailboxResponse,
) -> Result<CommitMailboxBatch, ConversationError> {
    validate_page(&page)?;
    let mut processed_epochs = Vec::new();
    let mut acknowledged_epochs = Vec::new();
    let mut membership_changes = Vec::new();
    let mut rejected_commit = None;
    for (entry_index, entry) in page.commits.into_iter().enumerate() {
        let epoch = entry.epoch;
        let result = process_group_entry(conversation, device, group_id, participants, entry);
        match result {
            Ok((newly_processed, membership_change)) => {
                if newly_processed {
                    processed_epochs.push(epoch);
                }
                if let Some(change) = membership_change {
                    membership_changes.push(change);
                }
                acknowledged_epochs.push(epoch);
            }
            Err(error) => {
                rejected_commit = Some(RejectedMailboxCommit {
                    entry_index,
                    epoch,
                    error,
                });
                break;
            }
        }
    }
    let pending_acknowledgment = (!acknowledged_epochs.is_empty()).then(|| AckE2eeCommitsRequest {
        device_id: device.device_id().to_string(),
        epochs: acknowledged_epochs,
    });
    Ok(CommitMailboxBatch {
        processed_epochs,
        pending_acknowledgment,
        rejected_commit,
        membership_changes,
    })
}

fn process_group_entry(
    conversation: &mut Option<MlsConversation>,
    device: &MlsDevice,
    group_id: GroupId,
    participants: &[PinnedUserIdentity],
    entry: E2eeCommitMailboxEntry,
) -> Result<(bool, Option<AuthenticatedMembershipChange>), ConversationError> {
    let committer_device_id = DeviceId::try_from(entry.committer_device_id)
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let commit = EncryptedGroupCommit {
        group_id,
        prior_epoch: entry.prior_epoch,
        epoch: entry.epoch,
        committer_device_id,
        commit_blob: entry.commit_blob,
    };
    let Some(current) = conversation.as_mut() else {
        let welcome = entry.welcome_blob.ok_or(ConversationError::InvalidCommit)?;
        validate_commit_envelope(&commit)?;
        let joined =
            MlsConversation::join_group_from_welcome(group_id, device, participants, &welcome)?;
        if joined.epoch() != entry.epoch
            || !joined.has_verified_member_device(committer_device_id)?
            || committer_device_id == device.device_id()
        {
            return Err(ConversationError::MetadataMismatch);
        }
        *conversation = Some(joined);
        return Ok((true, None));
    };
    if entry.epoch <= current.epoch() {
        return Ok((false, None));
    }
    if entry.welcome_blob.is_some() {
        return Err(ConversationError::UnexpectedMembership);
    }
    let authenticated = match entry.membership_change {
        Some(MlsMembershipChange::Add { leaf }) => {
            let target_user =
                UserId::try_from(leaf.user_id).map_err(|_| ConversationError::MetadataMismatch)?;
            let target_device = DeviceId::try_from(leaf.device_id)
                .map_err(|_| ConversationError::MetadataMismatch)?;
            let pin = participants
                .iter()
                .find(|pin| pin.user_id == target_user)
                .copied()
                .ok_or(ConversationError::UntrustedCredential)?;
            let change = current.process_incoming_participant_add_expected(
                device,
                &commit,
                pin,
                Some(target_device),
            )?;
            Some(change)
        }
        Some(MlsMembershipChange::Remove { leaves }) => {
            let expected = leaves
                .into_iter()
                .map(|leaf| {
                    Ok((
                        UserId::try_from(leaf.user_id)
                            .map_err(|_| ConversationError::MetadataMismatch)?,
                        DeviceId::try_from(leaf.device_id)
                            .map_err(|_| ConversationError::MetadataMismatch)?,
                    ))
                })
                .collect::<Result<Vec<_>, ConversationError>>()?;
            let expected_user_id = expected
                .first()
                .map(|(user_id, _)| *user_id)
                .ok_or(ConversationError::MetadataMismatch)?;
            if expected
                .iter()
                .any(|(user_id, _)| *user_id != expected_user_id)
            {
                return Err(ConversationError::MetadataMismatch);
            }
            let expected_device_ids = expected
                .into_iter()
                .map(|(_, device_id)| device_id)
                .collect::<Vec<_>>();
            let change = current.process_incoming_expected_remove(
                device,
                &commit,
                expected_user_id,
                &expected_device_ids,
            )?;
            Some(change)
        }
        None => {
            let change = current.process_incoming_commit_with_membership(device, &commit)?;
            if change.is_some() {
                return Err(ConversationError::MetadataMismatch);
            }
            None
        }
    };
    Ok((true, authenticated))
}

fn process_entry(
    conversation: &mut Option<MlsConversation>,
    device: &MlsDevice,
    group_id: GroupId,
    peer: PinnedUserIdentity,
    entry: E2eeCommitMailboxEntry,
) -> Result<bool, ConversationError> {
    let committer_device_id = DeviceId::try_from(entry.committer_device_id)
        .map_err(|_| ConversationError::MetadataMismatch)?;
    let commit = EncryptedGroupCommit {
        group_id,
        prior_epoch: entry.prior_epoch,
        epoch: entry.epoch,
        committer_device_id,
        commit_blob: entry.commit_blob,
    };

    if let Some(current) = conversation.as_mut() {
        if entry.epoch <= current.epoch() {
            return Ok(false);
        }
        if entry.welcome_blob.is_some() {
            return Err(ConversationError::UnexpectedMembership);
        }
        current.process_incoming_commit(device, &commit)?;
        return Ok(true);
    }

    let welcome = entry.welcome_blob.ok_or(ConversationError::InvalidCommit)?;
    validate_commit_envelope(&commit)?;
    let joined = MlsConversation::join_from_welcome(group_id, device, peer, &welcome)?;
    if joined.epoch() != entry.epoch
        || !joined.has_verified_member_device(committer_device_id)?
        || committer_device_id == device.device_id()
    {
        return Err(ConversationError::MetadataMismatch);
    }
    *conversation = Some(joined);
    Ok(true)
}

pub(crate) fn validate_page(page: &E2eeCommitMailboxResponse) -> Result<(), ConversationError> {
    if page.commits.len() > MAX_E2EE_COMMIT_MAILBOX_PAGE_SIZE {
        return Err(ConversationError::InvalidMailboxPage);
    }
    let aggregate_bytes = page.commits.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.commit_blob.len())?
            .checked_add(entry.welcome_blob.as_ref().map_or(0, Vec::len))
    });
    if aggregate_bytes.is_none_or(|total| total > MAX_E2EE_COMMIT_MAILBOX_PAGE_BLOB_BYTES) {
        return Err(ConversationError::InvalidMailboxPage);
    }

    let mut epochs = HashSet::with_capacity(page.commits.len());
    let mut previous_epoch = None;
    for entry in &page.commits {
        if entry.epoch == 0
            || entry.prior_epoch.checked_add(1) != Some(entry.epoch)
            || !epochs.insert(entry.epoch)
            || previous_epoch.is_some_and(|previous| entry.prior_epoch != previous)
            || DeviceId::try_from(entry.committer_device_id.clone()).is_err()
            || entry.commit_blob.is_empty()
            || entry.commit_blob.len() > MAX_COMMIT_BYTES
            || entry
                .welcome_blob
                .as_ref()
                .is_some_and(|blob| blob.is_empty() || blob.len() > MAX_WELCOME_BYTES)
            || entry.created_at_unix < 0
            || entry.expires_at_unix <= entry.created_at_unix
        {
            return Err(ConversationError::InvalidMailboxPage);
        }
        previous_epoch = Some(entry.epoch);
    }

    match (page.commits.last(), page.next_after_epoch) {
        (None, None) => Ok(()),
        (Some(last), Some(cursor)) if cursor == last.epoch => Ok(()),
        _ => Err(ConversationError::InvalidMailboxPage),
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{DeviceId, GroupId, UserId};
    use filament_protocol::MlsLeafRouting;

    use super::*;
    use crate::{generate_key_package_batch, RootIdentityKey};

    struct JoinedFixture {
        alice: MlsDevice,
        alice_group: MlsConversation,
        bob: MlsDevice,
        bob_group: MlsConversation,
        bob_pin: PinnedUserIdentity,
        group_id: GroupId,
    }

    fn joined_fixture() -> JoinedFixture {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let bob_keypackage = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, pending) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_keypackage).unwrap();
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
            bob_pin,
            group_id,
        }
    }

    fn mailbox_entry(pending: &crate::PendingGroupCommit) -> E2eeCommitMailboxEntry {
        E2eeCommitMailboxEntry {
            epoch: pending.epoch,
            prior_epoch: pending.prior_epoch,
            committer_device_id: pending.committer_device_id.to_string(),
            commit_blob: pending.commit_blob.clone(),
            welcome_blob: pending.welcome_blob.clone(),
            membership_change: None,
            created_at_unix: 10,
            expires_at_unix: 20,
        }
    }

    fn page(entries: Vec<E2eeCommitMailboxEntry>) -> E2eeCommitMailboxResponse {
        E2eeCommitMailboxResponse {
            next_after_epoch: entries.last().map(|entry| entry.epoch),
            commits: entries,
        }
    }

    #[test]
    fn recipient_bound_welcome_joins_and_builds_ack() {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let alice_pin = PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public());
        let bob_pin = PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public());
        let bob_keypackage = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_group, pending) =
            MlsConversation::create_two_member(group_id, &alice, bob_pin, &bob_keypackage).unwrap();
        let initial_page = page(vec![mailbox_entry(&pending)]);
        alice_group.accept_pending_commit(&alice).unwrap();

        let mut bob_state = None;
        let batch = process_commit_mailbox(&mut bob_state, &bob, group_id, alice_pin, initial_page)
            .unwrap();
        assert_eq!(batch.processed_epochs, vec![1]);
        assert!(batch.rejected_commit.is_none());
        assert_eq!(
            batch.pending_acknowledgment.unwrap(),
            AckE2eeCommitsRequest {
                device_id: bob.device_id().to_string(),
                epochs: vec![1],
            }
        );
        let bob_group = bob_state.as_mut().unwrap();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"joined from offline mailbox")
            .unwrap();
        assert_eq!(
            bob_group
                .decrypt_application_message(&bob, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"joined from offline mailbox"
        );
    }

    #[test]
    fn ordered_peer_updates_advance_state_and_ack_success_prefix() {
        let JoinedFixture {
            alice,
            alice_group,
            bob,
            mut bob_group,
            bob_pin,
            group_id,
            ..
        } = joined_fixture();
        let first = bob_group.create_self_update(&bob).unwrap();
        assert!(first.welcome_blob.is_none());
        bob_group.accept_pending_commit(&bob).unwrap();
        let second = bob_group.create_self_update(&bob).unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();

        let mut alice_state = Some(alice_group);
        let batch = process_commit_mailbox(
            &mut alice_state,
            &alice,
            group_id,
            bob_pin,
            page(vec![mailbox_entry(&first), mailbox_entry(&second)]),
        )
        .unwrap();
        assert_eq!(batch.processed_epochs, vec![2, 3]);
        assert_eq!(batch.pending_acknowledgment.unwrap().epochs, vec![2, 3]);
        assert_eq!(alice_state.as_ref().unwrap().epoch(), 3);

        let encrypted = bob_group
            .encrypt_application_message(&bob, b"after two commits")
            .unwrap();
        let outcome = alice_state
            .as_mut()
            .unwrap()
            .decrypt_application_message(&alice, &encrypted)
            .unwrap();
        assert_eq!(outcome.ready_messages[0].plaintext, b"after two commits");
    }

    #[test]
    fn added_device_joins_while_existing_device_processes_same_commit_without_welcome() {
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
        let bob_group = MlsConversation::join_from_welcome(
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
        let target_page = page(vec![mailbox_entry(&add)]);
        let mut existing_entry = mailbox_entry(&add);
        existing_entry.welcome_blob = None;
        let existing_page = page(vec![existing_entry]);
        alice_group.accept_pending_commit(&alice).unwrap();

        let mut bob_state = Some(bob_group);
        let existing_batch =
            process_commit_mailbox(&mut bob_state, &bob, group_id, alice_pin, existing_page)
                .unwrap();
        assert_eq!(existing_batch.processed_epochs, vec![2]);
        assert_eq!(
            existing_batch.pending_acknowledgment.unwrap().epochs,
            vec![2]
        );

        let mut bob_second_state = None;
        let target_batch = process_commit_mailbox(
            &mut bob_second_state,
            &bob_second,
            group_id,
            alice_pin,
            target_page,
        )
        .unwrap();
        assert_eq!(target_batch.processed_epochs, vec![2]);
        assert_eq!(target_batch.pending_acknowledgment.unwrap().epochs, vec![2]);

        let encrypted = alice_group
            .encrypt_application_message(&alice, b"offline device joined")
            .unwrap();
        assert_eq!(
            bob_state
                .as_mut()
                .unwrap()
                .decrypt_application_message(&bob, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"offline device joined"
        );
        assert_eq!(
            bob_second_state
                .as_mut()
                .unwrap()
                .decrypt_application_message(&bob_second, &encrypted)
                .unwrap()
                .ready_messages[0]
                .plaintext,
            b"offline device joined"
        );
    }

    #[test]
    fn forged_committer_hint_blocks_epoch_without_ack_or_merge() {
        let JoinedFixture {
            alice,
            alice_group,
            bob,
            mut bob_group,
            bob_pin,
            group_id,
            ..
        } = joined_fixture();
        let pending = bob_group.create_self_update(&bob).unwrap();
        bob_group.accept_pending_commit(&bob).unwrap();
        let mut forged = mailbox_entry(&pending);
        forged.committer_device_id = alice.device_id().to_string();
        let mut alice_state = Some(alice_group);

        let batch = process_commit_mailbox(
            &mut alice_state,
            &alice,
            group_id,
            bob_pin,
            page(vec![forged]),
        )
        .unwrap();
        assert!(batch.pending_acknowledgment.is_none());
        assert!(batch.processed_epochs.is_empty());
        assert_eq!(alice_state.as_ref().unwrap().epoch(), 1);
        assert_eq!(
            batch.rejected_commit.unwrap().error,
            ConversationError::MetadataMismatch
        );

        let retry = process_commit_mailbox(
            &mut alice_state,
            &alice,
            group_id,
            bob_pin,
            page(vec![mailbox_entry(&pending)]),
        )
        .unwrap();
        assert_eq!(retry.processed_epochs, vec![2]);
        assert_eq!(alice_state.as_ref().unwrap().epoch(), 2);
    }

    #[test]
    fn invalid_page_is_rejected_before_state_changes() {
        let JoinedFixture {
            alice,
            alice_group,
            bob,
            mut bob_group,
            bob_pin,
            group_id,
            ..
        } = joined_fixture();
        let pending = bob_group.create_self_update(&bob).unwrap();
        let mut invalid = page(vec![mailbox_entry(&pending)]);
        invalid.next_after_epoch = Some(99);
        let mut alice_state = Some(alice_group);

        assert_eq!(
            process_commit_mailbox(&mut alice_state, &alice, group_id, bob_pin, invalid,).err(),
            Some(ConversationError::InvalidMailboxPage)
        );
        assert_eq!(alice_state.as_ref().unwrap().epoch(), 1);
        assert!(bob_group.reject_pending_commit(&bob).is_ok());
    }

    #[test]
    fn durable_replay_epoch_is_acknowledged_without_reprocessing() {
        let JoinedFixture {
            alice,
            alice_group,
            bob_pin,
            group_id,
            ..
        } = joined_fixture();
        let replay = E2eeCommitMailboxEntry {
            epoch: 1,
            prior_epoch: 0,
            committer_device_id: DeviceId::new().to_string(),
            commit_blob: vec![0xFF],
            welcome_blob: Some(vec![0xFF]),
            membership_change: None,
            created_at_unix: 10,
            expires_at_unix: 20,
        };
        let mut alice_state = Some(alice_group);
        let batch = process_commit_mailbox(
            &mut alice_state,
            &alice,
            group_id,
            bob_pin,
            page(vec![replay]),
        )
        .unwrap();
        assert!(batch.processed_epochs.is_empty());
        assert_eq!(batch.pending_acknowledgment.unwrap().epochs, vec![1]);
        assert_eq!(alice_state.as_ref().unwrap().epoch(), 1);
    }

    #[test]
    fn group_mailbox_surfaces_only_authenticated_remove() {
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
        let removal = alice_group
            .create_remove_participant(&alice, charlie.user_id())
            .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let entry = E2eeCommitMailboxEntry {
            epoch: removal.epoch,
            prior_epoch: removal.prior_epoch,
            committer_device_id: removal.committer_device_id.to_string(),
            commit_blob: removal.commit_blob,
            welcome_blob: None,
            membership_change: Some(MlsMembershipChange::Remove {
                leaves: vec![MlsLeafRouting {
                    leaf_index: 2,
                    user_id: charlie.user_id().to_string(),
                    device_id: charlie.device_id().to_string(),
                }],
            }),
            created_at_unix: 10,
            expires_at_unix: 20,
        };
        let mut state = Some(bob_group);
        let mut mismatched = entry.clone();
        mismatched.membership_change = Some(MlsMembershipChange::Remove {
            leaves: vec![MlsLeafRouting {
                leaf_index: 2,
                user_id: charlie.user_id().to_string(),
                device_id: DeviceId::new().to_string(),
            }],
        });
        let rejected = process_group_commit_mailbox(
            &mut state,
            &bob,
            group_id,
            &[alice_pin, charlie_pin],
            page(vec![mismatched]),
        )
        .unwrap();
        assert!(rejected.rejected_commit.is_some());
        assert_eq!(state.as_ref().unwrap().epoch(), 1);
        let batch = process_group_commit_mailbox(
            &mut state,
            &bob,
            group_id,
            &[alice_pin, charlie_pin],
            page(vec![entry]),
        )
        .unwrap();
        assert!(batch.rejected_commit.is_none());
        assert_eq!(batch.membership_changes.len(), 1);
        assert_eq!(
            batch.membership_changes[0].target_user_id,
            charlie.user_id()
        );
        assert_eq!(batch.pending_acknowledgment.unwrap().epochs, vec![2]);
    }
}
