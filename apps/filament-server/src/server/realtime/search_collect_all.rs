use std::collections::HashMap;

use crate::server::core::{GuildRecord, IndexedMessage};

pub(crate) fn collect_all_indexed_messages_in_memory(
    guilds: &HashMap<String, GuildRecord>,
) -> Vec<IndexedMessage> {
    let mut docs = Vec::new();
    for (guild_id, guild) in guilds {
        for (channel_id, channel) in &guild.channels {
            for message in &channel.messages {
                docs.push(IndexedMessage {
                    message_id: message.id.clone(),
                    guild_id: guild_id.clone(),
                    channel_id: channel_id.clone(),
                    author_id: message.author_id.to_string(),
                    content: message.content.clone(),
                    created_at_unix: message.created_at_unix,
                });
            }
        }
    }
    docs
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use filament_core::{ChannelKind, Role, UserId};

    use super::collect_all_indexed_messages_in_memory;
    use crate::server::core::{ChannelRecord, GuildRecord, GuildVisibility, MessageRecord};

    #[test]
    fn collect_all_indexed_messages_returns_documents_for_all_channels() {
        let author = UserId::new();
        let guild_id = String::from("g1");
        let mut guilds = HashMap::new();
        guilds.insert(
            guild_id.clone(),
            GuildRecord {
                name: String::from("Guild"),
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
                                created_at_unix: 10,
                                reactions: HashMap::new(),
                            }],
                            role_overrides: HashMap::new(),
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
                                created_at_unix: 11,
                                reactions: HashMap::new(),
                            }],
                            role_overrides: HashMap::new(),
                        },
                    ),
                ]),
            },
        );

        let docs = collect_all_indexed_messages_in_memory(&guilds);

        assert_eq!(docs.len(), 2);
        assert!(docs.iter().any(|doc| {
            doc.message_id == "m1"
                && doc.guild_id == "g1"
                && doc.channel_id == "c1"
                && doc.content == "hello"
        }));
        assert!(docs.iter().any(|doc| {
            doc.message_id == "m2"
                && doc.guild_id == "g1"
                && doc.channel_id == "c2"
                && doc.content == "world"
        }));
    }
}