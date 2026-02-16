use std::collections::HashMap;

use filament_core::{MarkdownToken, UserId};

use crate::server::{
    core::MessageRecord,
    types::{AttachmentResponse, MessageResponse, ReactionResponse},
};

pub(crate) fn build_in_memory_message_record(
    message_id: String,
    author_id: UserId,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    attachment_ids: Vec<String>,
    created_at_unix: i64,
) -> MessageRecord {
    MessageRecord {
        id: message_id,
        author_id,
        content,
        markdown_tokens,
        attachment_ids,
        created_at_unix,
        reactions: HashMap::new(),
    }
}

pub(crate) fn build_message_response_from_record(
    record: &MessageRecord,
    guild_id: &str,
    channel_id: &str,
    attachments: Vec<AttachmentResponse>,
    reactions: Vec<ReactionResponse>,
) -> MessageResponse {
    MessageResponse {
        message_id: record.id.clone(),
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.to_owned(),
        author_id: record.author_id.to_string(),
        content: record.content.clone(),
        markdown_tokens: record.markdown_tokens.clone(),
        attachments,
        reactions,
        created_at_unix: record.created_at_unix,
    }
}

#[cfg(test)]
mod tests {
    use filament_core::{MarkdownToken, UserId};

    use super::{build_in_memory_message_record, build_message_response_from_record};
    use crate::server::types::{AttachmentResponse, ReactionResponse};

    #[test]
    fn builds_message_record_with_empty_reactions() {
        let record = build_in_memory_message_record(
            String::from("m1"),
            UserId::new(),
            String::from("hello"),
            vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            vec![String::from("a1")],
            42,
        );

        assert_eq!(record.id, "m1");
        assert_eq!(record.content, "hello");
        assert_eq!(record.attachment_ids, vec![String::from("a1")]);
        assert!(record.reactions.is_empty());
        assert_eq!(record.created_at_unix, 42);
    }

    #[test]
    fn builds_message_response_from_record_fields() {
        let author_id = UserId::new();
        let record = build_in_memory_message_record(
            String::from("m2"),
            author_id,
            String::from("content"),
            vec![MarkdownToken::Text {
                text: String::from("content"),
            }],
            vec![],
            99,
        );

        let attachments = vec![AttachmentResponse {
            attachment_id: String::from("a1"),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            owner_id: author_id.to_string(),
            filename: String::from("file.txt"),
            mime_type: String::from("text/plain"),
            size_bytes: 3,
            sha256_hex: String::from("abc"),
        }];
        let reactions = vec![ReactionResponse {
            emoji: String::from("🔥"),
            count: 2,
        }];

        let response =
            build_message_response_from_record(&record, "g1", "c1", attachments.clone(), reactions.clone());

        assert_eq!(response.message_id, "m2");
        assert_eq!(response.guild_id, "g1");
        assert_eq!(response.channel_id, "c1");
        assert_eq!(response.author_id, author_id.to_string());
        assert_eq!(response.content, "content");
        assert_eq!(response.attachments.len(), attachments.len());
        assert_eq!(response.reactions.len(), reactions.len());
        assert_eq!(response.created_at_unix, 99);
    }
}
