use std::collections::HashMap;

use crate::server::{
    core::{GuildRecord, MessageRecord},
    errors::AuthFailure,
};

pub(crate) fn append_message_record(
    guilds: &mut HashMap<String, GuildRecord>,
    guild_id: &str,
    channel_id: &str,
    record: MessageRecord,
) -> Result<(), AuthFailure> {
    let guild = guilds.get_mut(guild_id).ok_or(AuthFailure::NotFound)?;
    let channel = guild
        .channels
        .get_mut(channel_id)
        .ok_or(AuthFailure::NotFound)?;
    channel.messages.push(record);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::{ChannelKind, UserId};

    use super::append_message_record;
    use crate::server::{
        core::{
            ChannelRecord, GuildRecord, GuildVisibility, MessageRecord,
        },
        errors::AuthFailure,
    };

    fn sample_record() -> MessageRecord {
        MessageRecord {
            id: String::from("m1"),
            author_id: UserId::new(),
            content: String::from("hello"),
            markdown_tokens: Vec::new(),
            attachment_ids: Vec::new(),
            created_at_unix: 1,
            reactions: HashMap::new(),
        }
    }

    #[test]
    fn append_message_record_pushes_to_target_channel() {
        let mut guilds = HashMap::new();
        let mut guild = GuildRecord {
            name: String::from("Guild"),
            visibility: GuildVisibility::Private,
            created_by_user_id: UserId::new(),
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.channels.insert(
            String::from("c1"),
            ChannelRecord {
                name: String::from("general"),
                kind: ChannelKind::Text,
                messages: Vec::new(),
                role_overrides: HashMap::new(),
            },
        );
        guilds.insert(String::from("g1"), guild);

        append_message_record(&mut guilds, "g1", "c1", sample_record())
            .expect("append should succeed");

        let channel = &guilds["g1"].channels["c1"];
        assert_eq!(channel.messages.len(), 1);
        assert_eq!(channel.messages[0].id, "m1");
    }

    #[test]
    fn append_message_record_rejects_unknown_guild_or_channel() {
        let mut guilds = HashMap::new();
        let error = append_message_record(&mut guilds, "missing", "c1", sample_record())
            .expect_err("missing guild should fail closed");
        assert!(matches!(error, AuthFailure::NotFound));

        let mut guild = GuildRecord {
            name: String::from("Guild"),
            visibility: GuildVisibility::Private,
            created_by_user_id: UserId::new(),
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.channels.insert(
            String::from("other"),
            ChannelRecord {
                name: String::from("other"),
                kind: ChannelKind::Text,
                messages: Vec::new(),
                role_overrides: HashMap::new(),
            },
        );
        guilds.insert(String::from("g1"), guild);

        let error = append_message_record(&mut guilds, "g1", "missing", sample_record())
            .expect_err("missing channel should fail closed");
        assert!(matches!(error, AuthFailure::NotFound));
    }
}
