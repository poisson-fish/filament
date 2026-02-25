use std::collections::HashMap;

use crate::server::{
    core::{GuildRecord, IndexedMessage},
    errors::AuthFailure,
};

pub(crate) fn collect_indexed_messages_for_guild_in_memory(
    guilds: &HashMap<String, GuildRecord>,
    guild_id: &str,
    max_docs: usize,
) -> Result<Vec<IndexedMessage>, AuthFailure> {
    let Some(guild) = guilds.get(guild_id) else {
        return Err(AuthFailure::NotFound);
    };

    let mut docs = Vec::new();
    for (channel_id, channel) in &guild.channels {
        for message in &channel.messages {
            if docs.len() >= max_docs {
                return Err(AuthFailure::InvalidRequest);
            }
            docs.push(IndexedMessage {
                message_id: message.id.clone(),
                guild_id: guild_id.to_owned(),
                channel_id: channel_id.clone(),
                author_id: message.author_id.to_string(),
                content: message.content.clone(),
                created_at_unix: message.created_at_unix,
            });
        }
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, Role, UserId};

    use super::collect_indexed_messages_for_guild_in_memory;
    use crate::server::{
        core::{ChannelRecord, GuildRecord, GuildVisibility, MessageRecord},
        errors::AuthFailure,
    };

    fn guild_with_messages(guild_id: &str, message_ids: &[&str]) -> HashMap<String, GuildRecord> {
        let author = UserId::new();
        let messages = message_ids
            .iter()
            .map(|message_id| MessageRecord {
                id: (*message_id).to_owned(),
                author_id: author,
                content: format!("message-{message_id}"),
                markdown_tokens: Vec::new(),
                attachment_ids: Vec::new(),
                created_at_unix: 1,
                reactions: HashMap::new(),
            })
            .collect();

        HashMap::from([(
            guild_id.to_owned(),
            GuildRecord {
                name: String::from("Guild"),
                visibility: GuildVisibility::Private,
                created_by_user_id: author,
                default_join_role_id: None,
                members: HashMap::from([(author, Role::Owner)]),
                banned_members: HashSet::new(),
                channels: HashMap::from([(
                    String::from("c1"),
                    ChannelRecord {
                        name: String::from("general"),
                        kind: ChannelKind::Text,
                        messages,
                        role_overrides: HashMap::new(),
                    },
                )]),
            },
        )])
    }

    #[test]
    fn collect_indexed_messages_for_guild_returns_not_found_for_missing_guild() {
        let guilds = guild_with_messages("g1", &["m1"]);

        let result = collect_indexed_messages_for_guild_in_memory(&guilds, "missing", 10);

        assert!(matches!(result, Err(AuthFailure::NotFound)));
    }

    #[test]
    fn collect_indexed_messages_for_guild_rejects_when_cap_is_exceeded() {
        let guilds = guild_with_messages("g1", &["m1", "m2"]);

        let result = collect_indexed_messages_for_guild_in_memory(&guilds, "g1", 1);

        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));
    }

    #[test]
    fn collect_indexed_messages_for_guild_returns_all_messages_when_within_cap() {
        let guilds = guild_with_messages("g1", &["m1", "m2"]);

        let docs = collect_indexed_messages_for_guild_in_memory(&guilds, "g1", 2)
            .expect("documents should be collected");

        assert_eq!(docs.len(), 2);
        assert!(docs.iter().any(|doc| doc.message_id == "m1"));
        assert!(docs.iter().any(|doc| doc.message_id == "m2"));
    }
}
