use filament_core::{MarkdownToken, UserId};

use crate::server::types::{AttachmentResponse, MessageResponse};

pub(crate) fn build_db_created_message_response(
    message_id: String,
    guild_id: &str,
    channel_id: &str,
    author_id: UserId,
    content: String,
    markdown_tokens: Vec<MarkdownToken>,
    attachments: Vec<AttachmentResponse>,
    created_at_unix: i64,
) -> MessageResponse {
    MessageResponse {
        message_id,
        guild_id: guild_id.to_owned(),
        channel_id: channel_id.to_owned(),
        author_id: author_id.to_string(),
        content,
        markdown_tokens,
        attachments,
        reactions: Vec::new(),
        created_at_unix,
    }
}

#[cfg(test)]
mod tests {
    use super::build_db_created_message_response;
    use filament_core::{MarkdownToken, UserId};

    #[test]
    fn build_db_created_message_response_maps_all_fields_and_sets_empty_reactions() {
        let author = UserId::new();
        let response = build_db_created_message_response(
            String::from("m1"),
            "g1",
            "c1",
            author,
            String::from("content"),
            vec![MarkdownToken::Text {
                text: String::from("content"),
            }],
            Vec::new(),
            99,
        );

        assert_eq!(response.message_id, "m1");
        assert_eq!(response.guild_id, "g1");
        assert_eq!(response.channel_id, "c1");
        assert_eq!(response.author_id, author.to_string());
        assert_eq!(response.content, "content");
        assert_eq!(response.markdown_tokens.len(), 1);
        assert!(response.attachments.is_empty());
        assert!(response.reactions.is_empty());
        assert_eq!(response.created_at_unix, 99);
    }
}
