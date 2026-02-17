use std::collections::HashMap;

use crate::server::{
    core::GuildRecord, domain::reaction_summaries_from_users, errors::AuthFailure,
    types::MessageResponse,
};

pub(crate) fn collect_hydrated_messages_in_memory(
    guild: &GuildRecord,
    guild_id: &str,
    channel_id: Option<&str>,
) -> Result<HashMap<String, MessageResponse>, AuthFailure> {
    let mut by_id = HashMap::new();
    if let Some(channel_id) = channel_id {
        let channel = guild
            .channels
            .get(channel_id)
            .ok_or(AuthFailure::NotFound)?;
        for message in &channel.messages {
            by_id.insert(
                message.id.clone(),
                MessageResponse {
                    message_id: message.id.clone(),
                    guild_id: guild_id.to_owned(),
                    channel_id: channel_id.to_owned(),
                    author_id: message.author_id.to_string(),
                    content: message.content.clone(),
                    markdown_tokens: message.markdown_tokens.clone(),
                    attachments: Vec::new(),
                    reactions: reaction_summaries_from_users(&message.reactions),
                    created_at_unix: message.created_at_unix,
                },
            );
        }
        return Ok(by_id);
    }

    for (channel_id, channel) in &guild.channels {
        for message in &channel.messages {
            by_id.insert(
                message.id.clone(),
                MessageResponse {
                    message_id: message.id.clone(),
                    guild_id: guild_id.to_owned(),
                    channel_id: channel_id.clone(),
                    author_id: message.author_id.to_string(),
                    content: message.content.clone(),
                    markdown_tokens: message.markdown_tokens.clone(),
                    attachments: Vec::new(),
                    reactions: reaction_summaries_from_users(&message.reactions),
                    created_at_unix: message.created_at_unix,
                },
            );
        }
    }

    Ok(by_id)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, ChannelPermissionOverwrite, Role, UserId};

    use super::collect_hydrated_messages_in_memory;
    use crate::server::{
        core::{ChannelRecord, GuildRecord, GuildVisibility, MessageRecord},
        errors::AuthFailure,
    };

    fn guild_fixture(author: UserId) -> GuildRecord {
        GuildRecord {
            name: String::from("guild"),
            visibility: GuildVisibility::Private,
            created_by_user_id: author,
            members: HashMap::from([(author, Role::Owner)]),
            banned_members: HashSet::new(),
            channels: HashMap::from([
                (
                    String::from("c1"),
                    ChannelRecord {
                        name: String::from("general"),
                        kind: ChannelKind::Text,
                        messages: vec![MessageRecord {
                            id: String::from("m1"),
                            author_id: author,
                            content: String::from("hello"),
                            markdown_tokens: Vec::new(),
                            attachment_ids: Vec::new(),
                            created_at_unix: 11,
                            reactions: HashMap::new(),
                        }],
                        role_overrides: HashMap::<Role, ChannelPermissionOverwrite>::new(),
                    },
                ),
                (
                    String::from("c2"),
                    ChannelRecord {
                        name: String::from("random"),
                        kind: ChannelKind::Text,
                        messages: vec![MessageRecord {
                            id: String::from("m2"),
                            author_id: author,
                            content: String::from("world"),
                            markdown_tokens: Vec::new(),
                            attachment_ids: Vec::new(),
                            created_at_unix: 12,
                            reactions: HashMap::new(),
                        }],
                        role_overrides: HashMap::<Role, ChannelPermissionOverwrite>::new(),
                    },
                ),
            ]),
        }
    }

    #[test]
    fn returns_only_requested_channel_messages() {
        let author = UserId::new();
        let guild = guild_fixture(author);

        let by_id = collect_hydrated_messages_in_memory(&guild, "g1", Some("c1"))
            .expect("channel should exist");

        assert_eq!(by_id.len(), 1);
        let message = by_id.get("m1").expect("m1 should be present");
        assert_eq!(message.channel_id, "c1");
    }

    #[test]
    fn returns_all_channel_messages_when_channel_not_specified() {
        let author = UserId::new();
        let guild = guild_fixture(author);

        let by_id = collect_hydrated_messages_in_memory(&guild, "g1", None)
            .expect("all channels should be collected");

        assert_eq!(by_id.len(), 2);
        assert!(by_id.contains_key("m1"));
        assert!(by_id.contains_key("m2"));
    }

    #[test]
    fn fails_closed_when_requested_channel_missing() {
        let author = UserId::new();
        let guild = guild_fixture(author);

        let result = collect_hydrated_messages_in_memory(&guild, "g1", Some("missing"));

        assert!(matches!(result, Err(AuthFailure::NotFound)));
    }
}
