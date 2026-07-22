//! Strict application events carried inside MLS `PrivateMessage` ciphertext.
//!
//! The Delivery Service sees only an opaque, padded MLS record. Native clients
//! decode this versioned envelope after MLS authentication and render message
//! bodies through Filament's safe Markdown token model.

use filament_core::{tokenize_markdown, MarkdownToken};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use zeroize::Zeroizing;

use crate::{
    attachment::{validate_attachment_ids, AttachmentDescriptor, AttachmentKind},
    conversation::{EncryptedApplicationMessage, MlsConversation},
    error::ConversationError,
    keypackage::MlsDevice,
};

const APPLICATION_EVENT_VERSION: u16 = 1;
/// Maximum serialized application-event bytes before MLS framing and padding.
pub const MAX_APPLICATION_EVENT_BYTES: usize = 8 * 1_024;
/// Maximum UTF-8 bytes in a new or edited chat message.
pub const MAX_CHAT_MESSAGE_BYTES: usize = 2_000;
/// Maximum Unicode scalar values in a reaction token.
pub const MAX_REACTION_CHARS: usize = 32;
/// Maximum UTF-8 bytes in a sender-authored reply preview.
pub const MAX_QUOTE_PREVIEW_BYTES: usize = 280;
/// Maximum original attachments referenced by one encrypted chat event.
pub const MAX_ATTACHMENTS_PER_EVENT: usize = 5;

macro_rules! application_id {
    ($name:ident, $error:literal) => {
        #[doc = $error]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Ulid);

        impl $name {
            /// Generate a new identifier locally.
            #[must_use]
            pub fn new() -> Self {
                Self(Ulid::new())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl TryFrom<String> for $name {
            type Error = ConversationError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                let parsed = Ulid::from_string(&value)
                    .map_err(|_| ConversationError::InvalidApplicationMessage)?;
                if parsed.to_string() != value {
                    return Err(ConversationError::InvalidApplicationMessage);
                }
                Ok(Self(parsed))
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0.to_string()
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }
    };
}

application_id!(
    ApplicationEventId,
    "Unique, client-generated application event identifier."
);
application_id!(
    EncryptedMessageId,
    "Unique, client-generated encrypted message identifier."
);

/// Bounded Markdown source for a new message or edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChatMessageBody(String);

impl ChatMessageBody {
    /// Access the bounded Markdown source.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert untrusted Markdown into Filament's HTML-free UI token model.
    #[must_use]
    pub fn safe_markdown_tokens(&self) -> Vec<MarkdownToken> {
        tokenize_markdown(&self.0)
    }
}

impl TryFrom<String> for ChatMessageBody {
    type Error = ConversationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_valid_application_text(&value, MAX_CHAT_MESSAGE_BYTES) {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        Ok(Self(value))
    }
}

impl From<ChatMessageBody> for String {
    fn from(value: ChatMessageBody) -> Self {
        value.0
    }
}

/// Bounded sender-authored excerpt shown with a reply.
///
/// This is display context, never evidence of the referenced message's author
/// or contents. Clients must treat it as part of the replying sender's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QuotePreview(String);

impl QuotePreview {
    /// Access the bounded preview source.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert the preview into Filament's HTML-free UI token model.
    #[must_use]
    pub fn safe_markdown_tokens(&self) -> Vec<MarkdownToken> {
        tokenize_markdown(&self.0)
    }
}

impl TryFrom<String> for QuotePreview {
    type Error = ConversationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_valid_application_text(&value, MAX_QUOTE_PREVIEW_BYTES) {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        Ok(Self(value))
    }
}

impl From<QuotePreview> for String {
    fn from(value: QuotePreview) -> Self {
        value.0
    }
}

fn is_valid_application_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

/// Bounded reaction token. It may contain an emoji sequence or short text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReactionToken(String);

impl ReactionToken {
    /// Access the validated reaction token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ReactionToken {
    type Error = ConversationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let char_count = value.chars().count();
        if value.trim().is_empty()
            || char_count > MAX_REACTION_CHARS
            || value.chars().any(char::is_control)
        {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        Ok(Self(value))
    }
}

impl From<ReactionToken> for String {
    fn from(value: ReactionToken) -> Self {
        value.0
    }
}

/// Optional reference carried by a reply message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyReference {
    /// Client-generated ID of the referenced encrypted message.
    pub target_message_id: EncryptedMessageId,
    /// Bounded sender-authored context, not an authenticated copy of the target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<QuotePreview>,
}

/// One encrypted file and its optional independently encrypted thumbnail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedAttachmentReference {
    /// Original file descriptor carried only inside MLS ciphertext.
    pub file: AttachmentDescriptor,
    /// Optional client-generated thumbnail descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<AttachmentDescriptor>,
}

/// Non-empty, bounded attachment set for one encrypted chat event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "Vec<EncryptedAttachmentReference>",
    into = "Vec<EncryptedAttachmentReference>"
)]
pub struct AttachmentSet(Vec<EncryptedAttachmentReference>);

impl AttachmentSet {
    /// Access the validated attachment references.
    #[must_use]
    pub fn as_slice(&self) -> &[EncryptedAttachmentReference] {
        &self.0
    }
}

impl TryFrom<Vec<EncryptedAttachmentReference>> for AttachmentSet {
    type Error = ConversationError;

    fn try_from(value: Vec<EncryptedAttachmentReference>) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > MAX_ATTACHMENTS_PER_EVENT {
            return Err(ConversationError::LimitExceeded);
        }
        if value.iter().any(|reference| {
            reference.file.kind != AttachmentKind::File
                || reference
                    .thumbnail
                    .as_ref()
                    .is_some_and(|thumbnail| thumbnail.kind != AttachmentKind::Thumbnail)
        }) {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        let descriptors = value.iter().flat_map(|reference| {
            core::iter::once(&reference.file).chain(reference.thumbnail.as_ref())
        });
        validate_attachment_ids(descriptors)
            .map_err(|_| ConversationError::InvalidApplicationMessage)?;
        Ok(Self(value))
    }
}

impl From<AttachmentSet> for Vec<EncryptedAttachmentReference> {
    fn from(value: AttachmentSet) -> Self {
        value.0
    }
}

/// Whether the sender is adding or removing its own reaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    /// Add the sender's reaction.
    Add,
    /// Remove the sender's reaction.
    Remove,
}

/// Authenticated chat semantics encrypted wholly inside MLS.
///
/// Link previews, typing indicators, and read receipts deliberately have no
/// variant in protocol v1. Unknown kinds and fields fail strict decoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EncryptedChatEvent {
    /// Create a message, optionally as a reply.
    Message {
        message_id: EncryptedMessageId,
        body: ChatMessageBody,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply: Option<ReplyReference>,
    },
    /// Create a message whose encrypted objects are fetched separately.
    Attachments {
        message_id: EncryptedMessageId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<ChatMessageBody>,
        attachments: AttachmentSet,
    },
    /// Replace the body of a message. Clients enforce same-author ownership.
    Edit {
        target_message_id: EncryptedMessageId,
        body: ChatMessageBody,
    },
    /// Delete a message. Clients enforce same-author or disclosed policy rights.
    Delete {
        target_message_id: EncryptedMessageId,
    },
    /// Add or remove the authenticated sender's reaction.
    Reaction {
        target_message_id: EncryptedMessageId,
        reaction: ReactionToken,
        action: ReactionAction,
    },
    /// Set the shared pin state for a message.
    Pin {
        target_message_id: EncryptedMessageId,
        pinned: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEventEnvelope {
    v: u16,
    event_id: ApplicationEventId,
    event: EncryptedChatEvent,
}

/// One versioned application event ready for MLS encryption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedApplicationEvent {
    /// Unique identifier used for local idempotence.
    pub event_id: ApplicationEventId,
    /// Chat operation hidden from the Delivery Service.
    pub event: EncryptedChatEvent,
}

impl VersionedApplicationEvent {
    /// Serialize a strict, bounded v1 envelope.
    ///
    /// # Errors
    /// Returns an error if serialization fails or exceeds the application cap.
    pub fn encode(&self) -> Result<Vec<u8>, ConversationError> {
        let bytes = serde_json::to_vec(&ApplicationEventEnvelope {
            v: APPLICATION_EVENT_VERSION,
            event_id: self.event_id,
            event: self.event.clone(),
        })
        .map_err(|_| ConversationError::SerializationFailed)?;
        if bytes.is_empty() || bytes.len() > MAX_APPLICATION_EVENT_BYTES {
            return Err(ConversationError::LimitExceeded);
        }
        Ok(bytes)
    }

    /// Strictly decode a bounded v1 event after MLS authentication.
    ///
    /// # Errors
    /// Rejects unknown versions, fields, event kinds, invalid IDs, and limits.
    pub fn decode(bytes: &[u8]) -> Result<Self, ConversationError> {
        if bytes.is_empty() || bytes.len() > MAX_APPLICATION_EVENT_BYTES {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        let envelope: ApplicationEventEnvelope = serde_json::from_slice(bytes)
            .map_err(|_| ConversationError::InvalidApplicationMessage)?;
        if envelope.v != APPLICATION_EVENT_VERSION {
            return Err(ConversationError::InvalidApplicationMessage);
        }
        Ok(Self {
            event_id: envelope.event_id,
            event: envelope.event,
        })
    }
}

impl MlsConversation {
    /// Serialize and MLS-encrypt one typed chat event.
    ///
    /// # Errors
    /// Applies all application-event and MLS conversation invariants.
    pub fn encrypt_chat_event(
        &mut self,
        device: &MlsDevice,
        event: &VersionedApplicationEvent,
    ) -> Result<EncryptedApplicationMessage, ConversationError> {
        let encoded = Zeroizing::new(event.encode()?);
        self.encrypt_application_message(device, &encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encrypt_attachment, encrypt_thumbnail, generate_key_package_batch, PinnedUserIdentity,
        RootIdentityKey,
    };
    use filament_core::{DeviceId, GroupId, UserId};

    fn body(value: &str) -> ChatMessageBody {
        ChatMessageBody::try_from(value.to_owned()).unwrap()
    }

    #[test]
    fn strict_round_trip_covers_all_message_adjacent_events() {
        let message_id = EncryptedMessageId::new();
        let (file, _) = encrypt_attachment("notes.bin", b"private attachment").unwrap();
        let (thumbnail, _) = encrypt_thumbnail("preview.bin", b"private preview").unwrap();
        let cases = [
            EncryptedChatEvent::Message {
                message_id,
                body: body("hello **group**"),
                reply: Some(ReplyReference {
                    target_message_id: EncryptedMessageId::new(),
                    preview: Some(QuotePreview::try_from("earlier message".to_owned()).unwrap()),
                }),
            },
            EncryptedChatEvent::Attachments {
                message_id: EncryptedMessageId::new(),
                body: Some(body("attached securely")),
                attachments: AttachmentSet::try_from(vec![EncryptedAttachmentReference {
                    file,
                    thumbnail: Some(thumbnail),
                }])
                .unwrap(),
            },
            EncryptedChatEvent::Edit {
                target_message_id: message_id,
                body: body("edited"),
            },
            EncryptedChatEvent::Delete {
                target_message_id: message_id,
            },
            EncryptedChatEvent::Reaction {
                target_message_id: message_id,
                reaction: ReactionToken::try_from("👍🏽".to_owned()).unwrap(),
                action: ReactionAction::Add,
            },
            EncryptedChatEvent::Pin {
                target_message_id: message_id,
                pinned: true,
            },
        ];

        for event in cases {
            let envelope = VersionedApplicationEvent {
                event_id: ApplicationEventId::new(),
                event,
            };
            assert_eq!(
                VersionedApplicationEvent::decode(&envelope.encode().unwrap()).unwrap(),
                envelope
            );
        }
    }

    #[test]
    fn attachment_sets_are_bounded_typed_and_unique() {
        assert_eq!(
            AttachmentSet::try_from(Vec::new()).unwrap_err(),
            ConversationError::LimitExceeded
        );

        let (file, _) = encrypt_attachment("file.bin", b"private").unwrap();
        assert_eq!(
            AttachmentSet::try_from(vec![
                EncryptedAttachmentReference {
                    file: file.clone(),
                    thumbnail: None,
                },
                EncryptedAttachmentReference {
                    file,
                    thumbnail: None,
                },
            ])
            .unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );

        let mut too_many = Vec::new();
        for index in 0..=MAX_ATTACHMENTS_PER_EVENT {
            let (file, _) = encrypt_attachment(format!("file-{index}.bin"), b"private").unwrap();
            too_many.push(EncryptedAttachmentReference {
                file,
                thumbnail: None,
            });
        }
        assert_eq!(
            AttachmentSet::try_from(too_many).unwrap_err(),
            ConversationError::LimitExceeded
        );
    }

    #[test]
    fn decoder_rejects_unknown_semantics_fields_and_noncanonical_ids() {
        let unknown_kind = format!(
            r#"{{"v":1,"event_id":"{}","event":{{"kind":"typing"}}}}"#,
            ApplicationEventId::new()
        );
        assert_eq!(
            VersionedApplicationEvent::decode(unknown_kind.as_bytes()).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );

        let message_id = EncryptedMessageId::new();
        let unknown_field = format!(
            r#"{{"v":1,"event_id":"{}","event":{{"kind":"delete","target_message_id":"{message_id}","server_plaintext":true}}}}"#,
            ApplicationEventId::new()
        );
        assert_eq!(
            VersionedApplicationEvent::decode(unknown_field.as_bytes()).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );

        let lowercase_id = message_id.to_string().to_ascii_lowercase();
        assert_eq!(
            EncryptedMessageId::try_from(lowercase_id).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
    }

    #[test]
    fn bodies_are_bounded_and_render_only_to_safe_tokens() {
        assert_eq!(
            ChatMessageBody::try_from(String::new()).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
        assert_eq!(
            ChatMessageBody::try_from("x".repeat(MAX_CHAT_MESSAGE_BYTES + 1)).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
        assert_eq!(
            ChatMessageBody::try_from(" \t\n".to_owned()).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
        assert_eq!(
            ChatMessageBody::try_from("nul\0byte".to_owned()).unwrap_err(),
            ConversationError::InvalidApplicationMessage
        );
        let tokens =
            body("<script>alert(1)</script> [x](javascript:alert(1))").safe_markdown_tokens();
        let serialized = serde_json::to_string(&tokens).unwrap();
        assert!(!serialized.contains("<script>"));
        assert!(!serialized.contains("javascript:"));
    }

    #[test]
    fn message_adjacent_events_round_trip_inside_three_member_mls_group() {
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
        let (mut alice_group, pending) = MlsConversation::create_group(
            group_id,
            &alice,
            &[(bob_pin, bob_package), (charlie_pin, charlie_package)],
        )
        .unwrap();
        alice_group.accept_pending_commit(&alice).unwrap();
        let mut charlie_group = MlsConversation::join_group_from_welcome(
            group_id,
            &charlie,
            &[alice_pin, bob_pin],
            pending.welcome_blob.as_deref().unwrap(),
        )
        .unwrap();

        let original_id = EncryptedMessageId::new();
        let reply_target = EncryptedMessageId::new();
        let (file, _) = encrypt_attachment("group.bin", b"group attachment").unwrap();
        let events = [
            EncryptedChatEvent::Message {
                message_id: original_id,
                body: body("group message"),
                reply: Some(ReplyReference {
                    target_message_id: reply_target,
                    preview: Some(QuotePreview::try_from("quoted context".to_owned()).unwrap()),
                }),
            },
            EncryptedChatEvent::Attachments {
                message_id: EncryptedMessageId::new(),
                body: None,
                attachments: AttachmentSet::try_from(vec![EncryptedAttachmentReference {
                    file,
                    thumbnail: None,
                }])
                .unwrap(),
            },
            EncryptedChatEvent::Reaction {
                target_message_id: original_id,
                reaction: ReactionToken::try_from("🔥".to_owned()).unwrap(),
                action: ReactionAction::Add,
            },
            EncryptedChatEvent::Edit {
                target_message_id: original_id,
                body: body("edited group message"),
            },
            EncryptedChatEvent::Pin {
                target_message_id: original_id,
                pinned: true,
            },
            EncryptedChatEvent::Delete {
                target_message_id: original_id,
            },
        ];

        for event in events {
            let expected = VersionedApplicationEvent {
                event_id: ApplicationEventId::new(),
                event,
            };
            let ciphertext = alice_group.encrypt_chat_event(&alice, &expected).unwrap();
            let outcome = charlie_group
                .decrypt_application_message(&charlie, &ciphertext)
                .unwrap();
            assert_eq!(outcome.ready_messages.len(), 1);
            let decoded =
                VersionedApplicationEvent::decode(&outcome.ready_messages[0].plaintext).unwrap();
            assert_eq!(decoded, expected);
            assert_eq!(outcome.ready_messages[0].sender_user_id, alice.user_id());
            assert_eq!(
                outcome.ready_messages[0].sender_device_id,
                alice.device_id()
            );
        }
    }
}
