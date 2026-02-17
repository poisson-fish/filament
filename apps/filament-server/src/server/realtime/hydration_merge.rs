use std::collections::HashMap;

use crate::server::types::{AttachmentResponse, MessageResponse, ReactionResponse};

pub(crate) fn merge_hydration_maps(
    by_id: &mut HashMap<String, MessageResponse>,
    attachment_map: &HashMap<String, Vec<AttachmentResponse>>,
    reaction_map: &HashMap<String, Vec<ReactionResponse>>,
) {
    for (id, message) in by_id {
        message.attachments = attachment_map.get(id).cloned().unwrap_or_default();
        message.reactions = reaction_map.get(id).cloned().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::MarkdownToken;

    use super::merge_hydration_maps;
    use crate::server::types::{AttachmentResponse, MessageResponse, ReactionResponse};

    fn message(message_id: &str) -> MessageResponse {
        MessageResponse {
            message_id: String::from(message_id),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            content: String::from("hello"),
            markdown_tokens: vec![MarkdownToken::Text {
                text: String::from("hello"),
            }],
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 1,
        }
    }

    fn attachment(attachment_id: &str) -> AttachmentResponse {
        AttachmentResponse {
            attachment_id: String::from(attachment_id),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            owner_id: String::from("u1"),
            filename: String::from("a.txt"),
            mime_type: String::from("text/plain"),
            size_bytes: 1,
            sha256_hex: String::from("abc"),
        }
    }

    #[test]
    fn applies_attachments_and_reactions_per_message_id() {
        let mut by_id = HashMap::from([
            (String::from("m1"), message("m1")),
            (String::from("m2"), message("m2")),
        ]);
        let attachment_map =
            HashMap::from([(String::from("m1"), vec![attachment("a1"), attachment("a2")])]);
        let reaction_map = HashMap::from([(
            String::from("m2"),
            vec![ReactionResponse {
                emoji: String::from("😀"),
                count: 3,
            }],
        )]);

        merge_hydration_maps(&mut by_id, &attachment_map, &reaction_map);

        assert_eq!(by_id.get("m1").expect("m1").attachments.len(), 2);
        assert_eq!(by_id.get("m1").expect("m1").reactions.len(), 0);
        assert_eq!(by_id.get("m2").expect("m2").attachments.len(), 0);
        assert_eq!(by_id.get("m2").expect("m2").reactions.len(), 1);
    }
}
