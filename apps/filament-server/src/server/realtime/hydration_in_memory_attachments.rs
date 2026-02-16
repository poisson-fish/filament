use std::collections::HashMap;

use crate::server::types::{AttachmentResponse, MessageResponse};

pub(crate) fn apply_hydration_attachments(
    by_id: &mut HashMap<String, MessageResponse>,
    attachment_map: &HashMap<String, Vec<AttachmentResponse>>,
) {
    for (id, message) in by_id {
        message.attachments = attachment_map.get(id).cloned().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::MarkdownToken;

    use super::apply_hydration_attachments;
    use crate::server::types::{AttachmentResponse, MessageResponse};

    fn sample_message(message_id: &str) -> MessageResponse {
        MessageResponse {
            message_id: String::from(message_id),
            guild_id: String::from("g"),
            channel_id: String::from("c"),
            author_id: String::from("a"),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 1,
        }
    }

    fn sample_attachment(message_id: &str, attachment_id: &str) -> AttachmentResponse {
        AttachmentResponse {
            attachment_id: String::from(attachment_id),
            guild_id: String::from("g"),
            channel_id: String::from("c"),
            owner_id: String::from("a"),
            filename: format!("{message_id}.txt"),
            mime_type: String::from("text/plain"),
            size_bytes: 10,
            sha256_hex: String::from("abc"),
        }
    }

    #[test]
    fn assigns_attachments_when_message_id_exists_in_map() {
        let mut by_id = HashMap::from([(String::from("m1"), sample_message("m1"))]);
        let attachment_map = HashMap::from([(
            String::from("m1"),
            vec![sample_attachment("m1", "att-1")],
        )]);

        apply_hydration_attachments(&mut by_id, &attachment_map);

        let message = by_id.get("m1").expect("message should exist");
        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].attachment_id, "att-1");
    }

    #[test]
    fn clears_existing_attachments_when_message_id_is_missing_from_map() {
        let mut message = sample_message("m1");
        message.attachments = vec![sample_attachment("m1", "att-stale")];
        let mut by_id = HashMap::from([(String::from("m1"), message)]);
        let attachment_map = HashMap::new();

        apply_hydration_attachments(&mut by_id, &attachment_map);

        let message = by_id.get("m1").expect("message should exist");
        assert!(message.attachments.is_empty());
    }
}
