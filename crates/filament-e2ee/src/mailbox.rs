//! Fail-closed client processing for offline MLS application mailboxes.
//!
//! A Delivery Service response is untrusted. Page invariants are checked
//! before MLS state is touched, each entry's routing hints are converted to
//! domain types, and only successfully authenticated/decrypted entries are
//! included in the acknowledgment request.

use std::collections::HashSet;

use filament_core::{CiphersuiteId, ConversationCrypto, DeviceId};
use filament_protocol::{
    AckE2eeMessagesRequest, E2eeMailboxMessage, E2eeMailboxResponse,
    MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES, MAX_E2EE_MAILBOX_PAGE_SIZE,
};
use ulid::Ulid;

use crate::{
    ConversationError, DecryptedApplicationMessage, EncryptedApplicationMessage, MlsConversation,
    MlsDevice,
};

const MESSAGE_TRANSPORT_PADDING_BUCKETS: [usize; 4] = [512, 1_024, 4_096, 16_384];
const MAX_UNIX_TIMESTAMP: i64 = 253_402_300_799;

/// One mailbox entry rejected without exposing attacker-controlled contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedMailboxMessage {
    /// Zero-based position in the bounded mailbox page.
    pub entry_index: usize,
    /// Fail-closed validation or MLS processing error.
    pub error: ConversationError,
}

/// MLS-authenticated plaintext bound to its validated transport identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedMailboxMessage {
    /// Canonical server transport ID used for durable history and acknowledgment.
    pub message_id: String,
    /// Bounded server receipt time retained only as untrusted display metadata.
    pub created_at_unix: i64,
    /// Sender and plaintext authenticated by MLS, independent of server hints.
    pub message: DecryptedApplicationMessage,
}

/// Results from processing one bounded offline mailbox page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailboxDecryptionBatch {
    /// Every newly MLS-authenticated plaintext in mailbox order. This includes
    /// messages held behind a generation gap and is the durable-persistence
    /// input for acknowledgment safety.
    pub authenticated_messages: Vec<AuthenticatedMailboxMessage>,
    /// Authenticated plaintext messages released in per-sender generation order.
    pub ready_messages: Vec<DecryptedApplicationMessage>,
    /// Request to send only after `authenticated_messages` and the updated MLS
    /// state are durably persisted.
    ///
    /// `None` means no mailbox entry was safe to acknowledge.
    pub pending_acknowledgment: Option<AckE2eeMessagesRequest>,
    /// Entries that were not acknowledged because validation or MLS processing failed.
    pub rejected_messages: Vec<RejectedMailboxMessage>,
    /// Whether authenticated later generations remain buffered behind a gap.
    pub messages_may_be_missing: bool,
}

/// Authenticate, decrypt, and generation-order a bounded mailbox page.
///
/// Page-level shape checks run before MLS state is consumed. Entry failures
/// are isolated so one malformed server record cannot suppress acknowledgments
/// for other records that were successfully authenticated. The caller must
/// durably persist `authenticated_messages` and the updated MLS state before
/// sending the returned acknowledgment.
///
/// # Errors
/// Returns [`ConversationError::InvalidMailboxPage`] if identifiers, cursors,
/// uniqueness, record count, or aggregate byte limits are invalid.
pub fn process_message_mailbox(
    conversation: &mut MlsConversation,
    device: &MlsDevice,
    page: E2eeMailboxResponse,
) -> Result<MailboxDecryptionBatch, ConversationError> {
    validate_page(&page)?;

    let mut authenticated_messages = Vec::new();
    let mut ready_messages = Vec::new();
    let mut acknowledged_ids = Vec::with_capacity(page.messages.len());
    let mut rejected_messages = Vec::new();

    for (entry_index, entry) in page.messages.into_iter().enumerate() {
        let message_id = entry.message_id.clone();
        let created_at_unix = entry.created_at_unix;
        match into_encrypted_message(conversation, entry)
            .and_then(|message| conversation.decrypt_application_message(device, &message))
        {
            Ok(outcome) => {
                authenticated_messages.push(AuthenticatedMailboxMessage {
                    message_id: message_id.clone(),
                    created_at_unix,
                    message: outcome.authenticated_message,
                });
                ready_messages.extend(outcome.ready_messages);
                acknowledged_ids.push(message_id);
            }
            Err(error) => rejected_messages.push(RejectedMailboxMessage { entry_index, error }),
        }
    }

    let pending_acknowledgment = (!acknowledged_ids.is_empty()).then(|| AckE2eeMessagesRequest {
        device_id: device.device_id().to_string(),
        message_ids: acknowledged_ids,
    });
    Ok(MailboxDecryptionBatch {
        authenticated_messages,
        ready_messages,
        pending_acknowledgment,
        rejected_messages,
        messages_may_be_missing: conversation.messages_may_be_missing(),
    })
}

fn validate_page(page: &E2eeMailboxResponse) -> Result<(), ConversationError> {
    if page.messages.len() > MAX_E2EE_MAILBOX_PAGE_SIZE {
        return Err(ConversationError::InvalidMailboxPage);
    }
    let aggregate_bytes = page.messages.iter().try_fold(0_usize, |total, entry| {
        total.checked_add(entry.message_blob.len())
    });
    if aggregate_bytes.is_none_or(|total| total > MAX_E2EE_MAILBOX_PAGE_BLOB_BYTES) {
        return Err(ConversationError::InvalidMailboxPage);
    }

    let mut message_ids = HashSet::with_capacity(page.messages.len());
    for entry in &page.messages {
        if !is_canonical_ulid(&entry.message_id)
            || !message_ids.insert(entry.message_id.as_str())
            || !MESSAGE_TRANSPORT_PADDING_BUCKETS.contains(&entry.message_blob.len())
            || !(0..=MAX_UNIX_TIMESTAMP).contains(&entry.created_at_unix)
            || entry.expires_at_unix <= entry.created_at_unix
        {
            return Err(ConversationError::InvalidMailboxPage);
        }
    }

    match (page.messages.last(), page.next_after_message_id.as_deref()) {
        (None, None) => Ok(()),
        (Some(last), Some(cursor)) if cursor == last.message_id && is_canonical_ulid(cursor) => {
            Ok(())
        }
        _ => Err(ConversationError::InvalidMailboxPage),
    }
}

fn into_encrypted_message(
    conversation: &MlsConversation,
    entry: E2eeMailboxMessage,
) -> Result<EncryptedApplicationMessage, ConversationError> {
    let crypto = ConversationCrypto::try_from(entry.crypto)
        .map_err(|_| ConversationError::CryptoModeMismatch)?;
    let suite =
        CiphersuiteId::try_from(entry.suite_id).map_err(|_| ConversationError::MetadataMismatch)?;
    if !is_canonical_ulid(&entry.sender_device_id) {
        return Err(ConversationError::MetadataMismatch);
    }
    let sender_device_id = DeviceId::try_from(entry.sender_device_id)
        .map_err(|_| ConversationError::MetadataMismatch)?;
    Ok(EncryptedApplicationMessage {
        crypto,
        group_id: conversation.group_id(),
        epoch: entry.epoch,
        suite,
        sender_device_id,
        message_blob: entry.message_blob,
    })
}

fn is_canonical_ulid(value: &str) -> bool {
    Ulid::from_string(value).is_ok_and(|parsed| parsed.to_string() == value)
}

#[cfg(test)]
mod tests {
    use filament_core::{DeviceId, GroupId, UserId};
    use filament_protocol::{E2eeMailboxMessage, E2eeMailboxResponse};

    use super::*;
    use crate::{
        generate_key_package_batch, MlsConversation, MlsDevice, PinnedUserIdentity, RootIdentityKey,
    };

    fn joined_conversations() -> (MlsDevice, MlsConversation, MlsDevice, MlsConversation) {
        let alice_root = RootIdentityKey::generate();
        let bob_root = RootIdentityKey::generate();
        let alice = MlsDevice::generate(UserId::new(), DeviceId::new(), &alice_root).unwrap();
        let bob = MlsDevice::generate(UserId::new(), DeviceId::new(), &bob_root).unwrap();
        let bob_keypackage = generate_key_package_batch(&bob, 1).unwrap().remove(0).blob;
        let group_id = GroupId::new();
        let (mut alice_conversation, pending) = MlsConversation::create_two_member(
            group_id,
            &alice,
            PinnedUserIdentity::new(bob.user_id(), *bob.root_key_public()),
            &bob_keypackage,
        )
        .unwrap();
        alice_conversation.accept_pending_commit(&alice).unwrap();
        let bob_conversation = MlsConversation::join_from_welcome(
            group_id,
            &bob,
            PinnedUserIdentity::new(alice.user_id(), *alice.root_key_public()),
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();
        (alice, alice_conversation, bob, bob_conversation)
    }

    fn mailbox_entry(
        message_id: String,
        encrypted: EncryptedApplicationMessage,
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
    fn offline_page_decrypts_out_of_order_and_builds_success_only_ack() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let first = alice_group
            .encrypt_application_message(&alice, b"offline zero")
            .unwrap();
        let second = alice_group
            .encrypt_application_message(&alice, b"offline one")
            .unwrap();
        assert!(MESSAGE_TRANSPORT_PADDING_BUCKETS.contains(&first.message_blob.len()));
        assert!(MESSAGE_TRANSPORT_PADDING_BUCKETS.contains(&second.message_blob.len()));

        let second_id = Ulid::new().to_string();
        let first_id = Ulid::new().to_string();
        let page = E2eeMailboxResponse {
            messages: vec![
                mailbox_entry(second_id.clone(), second),
                mailbox_entry(first_id.clone(), first),
            ],
            next_after_message_id: Some(first_id.clone()),
        };
        let encoded_page = serde_json::to_vec(&page).unwrap();
        let decoded_page = serde_json::from_slice(&encoded_page).unwrap();
        let batch = process_message_mailbox(&mut bob_group, &bob, decoded_page).unwrap();

        assert_eq!(batch.authenticated_messages.len(), 2);
        assert_eq!(
            batch
                .ready_messages
                .iter()
                .map(|message| message.plaintext.as_slice())
                .collect::<Vec<_>>(),
            vec![b"offline zero".as_slice(), b"offline one".as_slice()]
        );
        assert!(!batch.messages_may_be_missing);
        assert!(batch.rejected_messages.is_empty());
        let acknowledgment = batch.pending_acknowledgment.unwrap();
        assert_eq!(acknowledgment.device_id, bob.device_id().to_string());
        assert_eq!(acknowledgment.message_ids, vec![second_id, first_id]);
    }

    #[test]
    fn downgraded_entry_is_rejected_and_never_acknowledged() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let mut downgraded = mailbox_entry(
            Ulid::new().to_string(),
            alice_group
                .encrypt_application_message(&alice, b"do not release")
                .unwrap(),
        );
        downgraded.crypto = "plaintext".to_owned();
        let valid_id = Ulid::new().to_string();
        let valid = mailbox_entry(
            valid_id.clone(),
            alice_group
                .encrypt_application_message(&alice, b"release this")
                .unwrap(),
        );
        let page = E2eeMailboxResponse {
            messages: vec![downgraded, valid],
            next_after_message_id: Some(valid_id.clone()),
        };

        let batch = process_message_mailbox(&mut bob_group, &bob, page).unwrap();
        assert_eq!(batch.ready_messages.len(), 0);
        assert!(batch.messages_may_be_missing);
        assert_eq!(batch.rejected_messages.len(), 1);
        assert_eq!(
            batch.rejected_messages[0].error,
            ConversationError::CryptoModeMismatch
        );
        assert_eq!(
            batch.pending_acknowledgment.unwrap().message_ids,
            vec![valid_id]
        );
    }

    #[test]
    fn invalid_cursor_rejects_page_before_consuming_mls_state() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let encrypted = alice_group
            .encrypt_application_message(&alice, b"retry after bad page")
            .unwrap();
        let message_id = Ulid::new().to_string();
        let entry = mailbox_entry(message_id.clone(), encrypted);
        let invalid = E2eeMailboxResponse {
            messages: vec![entry.clone()],
            next_after_message_id: Some(Ulid::new().to_string()),
        };
        assert_eq!(
            process_message_mailbox(&mut bob_group, &bob, invalid).unwrap_err(),
            ConversationError::InvalidMailboxPage
        );

        let valid = E2eeMailboxResponse {
            messages: vec![entry],
            next_after_message_id: Some(message_id.clone()),
        };
        let batch = process_message_mailbox(&mut bob_group, &bob, valid).unwrap();
        assert_eq!(batch.ready_messages[0].plaintext, b"retry after bad page");
        assert_eq!(
            batch.pending_acknowledgment.unwrap().message_ids,
            vec![message_id]
        );
    }

    #[test]
    fn nonzero_transport_padding_is_rejected_without_ack() {
        let (alice, mut alice_group, bob, mut bob_group) = joined_conversations();
        let mut encrypted = alice_group
            .encrypt_application_message(&alice, b"authenticated body")
            .unwrap();
        *encrypted.message_blob.last_mut().unwrap() = 1;
        let message_id = Ulid::new().to_string();
        let page = E2eeMailboxResponse {
            messages: vec![mailbox_entry(message_id.clone(), encrypted)],
            next_after_message_id: Some(message_id),
        };
        let batch = process_message_mailbox(&mut bob_group, &bob, page).unwrap();
        assert!(batch.ready_messages.is_empty());
        assert!(batch.pending_acknowledgment.is_none());
        assert_eq!(batch.rejected_messages.len(), 1);
        assert_eq!(
            batch.rejected_messages[0].error,
            ConversationError::SerializationFailed
        );
    }
}
