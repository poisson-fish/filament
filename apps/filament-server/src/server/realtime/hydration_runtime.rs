use std::collections::HashMap;

use crate::server::{
    core::AppState,
    domain::{
        attachment_map_for_messages_db, attachment_map_for_messages_in_memory,
        reaction_map_for_messages_db,
    },
    errors::AuthFailure,
    types::MessageResponse,
};

use super::{
    apply_hydration_attachments, collect_hydrated_in_request_order, collect_hydrated_messages_db,
    collect_hydrated_messages_in_memory, merge_hydration_maps,
};

fn hydrate_in_request_order(
    by_id: HashMap<String, MessageResponse>,
    message_ids: &[String],
) -> Vec<MessageResponse> {
    collect_hydrated_in_request_order(by_id, message_ids)
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

        return Ok(hydrate_in_request_order(by_id, message_ids));
    }

    let guilds = state.guilds.read().await;
    let guild = guilds.get(guild_id).ok_or(AuthFailure::NotFound)?;
    let mut by_id = collect_hydrated_messages_in_memory(guild, guild_id, channel_id)?;

    let attachment_map =
        attachment_map_for_messages_in_memory(state, guild_id, channel_id, message_ids).await;
    apply_hydration_attachments(&mut by_id, &attachment_map);

    Ok(hydrate_in_request_order(by_id, message_ids))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::hydrate_in_request_order;
    use crate::server::types::MessageResponse;

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

        let hydrated = hydrate_in_request_order(by_id, &requested);
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

        let hydrated = hydrate_in_request_order(by_id, &requested);
        let ids: Vec<&str> = hydrated
            .iter()
            .map(|entry| entry.message_id.as_str())
            .collect();
        assert_eq!(ids, vec!["m1"]);
    }
}
