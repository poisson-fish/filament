use std::collections::HashMap;

use crate::server::{
    core::AppState,
    domain::{
        attachment_map_for_messages_db, attachment_map_for_messages_in_memory,
        reaction_map_for_messages_db,
    },
    errors::AuthFailure,
    types::{AttachmentResponse, MessageResponse, ReactionResponse},
};

use super::{collect_hydrated_messages_db, collect_hydrated_messages_in_memory};

pub(crate) fn collect_hydrated_in_request_order(
    by_id: HashMap<String, MessageResponse>,
    message_ids: &[String],
) -> Vec<MessageResponse> {
    let mut by_id = by_id;
    let mut hydrated = Vec::with_capacity(message_ids.len());
    for message_id in message_ids {
        if let Some(message) = by_id.remove(message_id) {
            hydrated.push(message);
        }
    }
    hydrated
}

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

pub(crate) fn apply_hydration_attachments(
    by_id: &mut HashMap<String, MessageResponse>,
    attachment_map: &HashMap<String, Vec<AttachmentResponse>>,
) {
    for (id, message) in by_id {
        message.attachments = attachment_map.get(id).cloned().unwrap_or_default();
    }
}

pub(crate) async fn hydrate_messages_by_id_runtime(
    state: &AppState,
    guild_id: &str,
    channel_id: Option<&str>,
    message_ids: &[String],
) -> Result<Vec<MessageResponse>, AuthFailure> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(pool) = &state.db_pool {
        let mut by_id =
            collect_hydrated_messages_db(pool, guild_id, channel_id, message_ids).await?;

        let message_ids_ordered: Vec<String> = message_ids.to_vec();
        let attachment_map =
            attachment_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered)
                .await?;
        let reaction_map =
            reaction_map_for_messages_db(pool, guild_id, channel_id, &message_ids_ordered).await?;
        merge_hydration_maps(&mut by_id, &attachment_map, &reaction_map);

        return Ok(collect_hydrated_in_request_order(by_id, message_ids));
    }

    let guilds = state.membership_store.guilds().read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut by_id = collect_hydrated_messages_in_memory(guild, guild_id, channel_id)?;

    let attachment_map =
        attachment_map_for_messages_in_memory(state, guild_id, channel_id, message_ids).await;
    apply_hydration_attachments(&mut by_id, &attachment_map);

    Ok(collect_hydrated_in_request_order(by_id, message_ids))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::MarkdownToken;

    use super::{
        apply_hydration_attachments, collect_hydrated_in_request_order, merge_hydration_maps,
    };
    use crate::server::types::{AttachmentResponse, MessageResponse, ReactionResponse};

    fn message(id: &str, content: &str) -> MessageResponse {
        MessageResponse {
            message_id: id.to_owned(),
            guild_id: String::from("g1"),
            channel_id: String::from("c1"),
            author_id: String::from("u1"),
            markdown_tokens: Vec::new(),
            content: content.to_owned(),
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at_unix: 1,
        }
    }

    #[test]
    fn hydrate_in_request_order_returns_messages_in_requested_order() {
        let mut by_id = HashMap::new();
        by_id.insert(String::from("m1"), message("m1", "first"));
        by_id.insert(String::from("m2"), message("m2", "second"));
        let requested = vec![String::from("m2"), String::from("m1")];

        let hydrated = collect_hydrated_in_request_order(by_id, &requested);
        let ids: Vec<&str> = hydrated
            .iter()
            .map(|entry| entry.message_id.as_str())
            .collect();
        assert_eq!(ids, vec!["m2", "m1"]);
    }

    #[test]
    fn hydrate_in_request_order_skips_missing_messages_fail_closed() {
        let mut by_id = HashMap::new();
        by_id.insert(String::from("m1"), message("m1", "first"));
        let requested = vec![String::from("missing"), String::from("m1")];

        let hydrated = collect_hydrated_in_request_order(by_id, &requested);
        let ids: Vec<&str> = hydrated
            .iter()
            .map(|entry| entry.message_id.as_str())
            .collect();
        assert_eq!(ids, vec!["m1"]);
    }

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
    fn applies_attachments_when_message_id_exists_in_map() {
        let mut by_id = HashMap::from([(String::from("m1"), sample_message("m1"))]);
        let attachment_map =
            HashMap::from([(String::from("m1"), vec![sample_attachment("m1", "att-1")])]);

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

    fn merge_message(message_id: &str) -> MessageResponse {
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

    fn merge_attachment(attachment_id: &str) -> AttachmentResponse {
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
    fn merge_hydration_applies_attachments_and_reactions_per_message_id() {
        let mut by_id = HashMap::from([
            (String::from("m1"), merge_message("m1")),
            (String::from("m2"), merge_message("m2")),
        ]);
        let attachment_map = HashMap::from([(
            String::from("m1"),
            vec![merge_attachment("a1"), merge_attachment("a2")],
        )]);
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
