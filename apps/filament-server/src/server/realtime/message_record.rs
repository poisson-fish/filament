use std::collections::HashMap;

use filament_core::{MarkdownToken, UserId};

use crate::server::{
    core::{AttachmentRecord, GuildRecord, MessageRecord},
    errors::AuthFailure,
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

#[allow(clippy::too_many_arguments)]
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

pub(crate) fn bind_message_attachments_in_memory(
    attachments: &mut HashMap<String, AttachmentRecord>,
    attachment_ids: &[String],
    message_id: &str,
    guild_id: &str,
    channel_id: &str,
    owner_id: UserId,
) -> Result<(), AuthFailure> {
    for attachment_id in attachment_ids {
        let Some(attachment) = attachments.get_mut(attachment_id) else {
            return Err(AuthFailure::InvalidRequest);
        };
        if attachment.guild_id != guild_id
            || attachment.channel_id != channel_id
            || attachment.owner_id != owner_id
            || attachment.message_id.is_some()
        {
            return Err(AuthFailure::InvalidRequest);
        }
        attachment.message_id = Some(message_id.to_owned());
    }
    Ok(())
}

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

    use filament_core::{MarkdownToken, UserId};

    use super::{
        append_message_record, bind_message_attachments_in_memory,
        build_db_created_message_response, build_in_memory_message_record,
        build_message_response_from_record,
    };
    use crate::server::{
        core::{AttachmentRecord, ChannelRecord, GuildRecord, GuildVisibility, MessageRecord},
        errors::AuthFailure,
        types::{AttachmentResponse, ReactionResponse},
    };

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
            reacted_by_me: false,
            reactor_user_ids: Vec::new(),
        }];

        let response = build_message_response_from_record(
            &record,
            "g1",
            "c1",
            attachments.clone(),
            reactions.clone(),
        );

        assert_eq!(response.message_id, "m2");
        assert_eq!(response.guild_id, "g1");
        assert_eq!(response.channel_id, "c1");
        assert_eq!(response.author_id, author_id.to_string());
        assert_eq!(response.content, "content");
        assert_eq!(response.attachments.len(), attachments.len());
        assert_eq!(response.reactions.len(), reactions.len());
        assert_eq!(response.created_at_unix, 99);
    }

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

    fn attachment(
        attachment_id: &str,
        guild_id: &str,
        channel_id: &str,
        owner_id: UserId,
        message_id: Option<&str>,
    ) -> AttachmentRecord {
        AttachmentRecord {
            attachment_id: String::from(attachment_id),
            guild_id: String::from(guild_id),
            channel_id: String::from(channel_id),
            owner_id,
            filename: String::from("file.png"),
            mime_type: String::from("image/png"),
            size_bytes: 12,
            sha256_hex: String::from("abc"),
            object_key: String::from("obj-1"),
            message_id: message_id.map(String::from),
        }
    }

    #[test]
    fn bind_message_attachments_in_memory_binds_when_constraints_match() {
        let owner_id = UserId::new();
        let mut attachments = HashMap::from([
            (
                String::from("a1"),
                attachment("a1", "g1", "c1", owner_id, None),
            ),
            (
                String::from("a2"),
                attachment("a2", "g1", "c1", owner_id, None),
            ),
        ]);

        bind_message_attachments_in_memory(
            &mut attachments,
            &[String::from("a1"), String::from("a2")],
            "m1",
            "g1",
            "c1",
            owner_id,
        )
        .expect("attachments should bind");

        assert_eq!(attachments["a1"].message_id.as_deref(), Some("m1"));
        assert_eq!(attachments["a2"].message_id.as_deref(), Some("m1"));
    }

    #[test]
    fn bind_message_attachments_in_memory_rejects_invalid_or_missing_attachment() {
        let owner_id = UserId::new();
        let mut attachments = HashMap::new();

        let result = bind_message_attachments_in_memory(
            &mut attachments,
            &[String::from("missing")],
            "m1",
            "g1",
            "c1",
            owner_id,
        );
        assert!(matches!(result, Err(AuthFailure::InvalidRequest)));

        let different_owner = UserId::new();
        let mut attachments = HashMap::from([
            (
                String::from("owned-by-other"),
                attachment("owned-by-other", "g1", "c1", different_owner, None),
            ),
            (
                String::from("already-bound"),
                attachment("already-bound", "g1", "c1", owner_id, Some("m0")),
            ),
        ]);

        let owner_result = bind_message_attachments_in_memory(
            &mut attachments,
            &[String::from("owned-by-other")],
            "m1",
            "g1",
            "c1",
            owner_id,
        );
        assert!(matches!(owner_result, Err(AuthFailure::InvalidRequest)));

        let bound_result = bind_message_attachments_in_memory(
            &mut attachments,
            &[String::from("already-bound")],
            "m1",
            "g1",
            "c1",
            owner_id,
        );
        assert!(matches!(bound_result, Err(AuthFailure::InvalidRequest)));
    }

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
            default_join_role_id: None,
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.channels.insert(
            String::from("c1"),
            ChannelRecord {
                name: String::from("general"),
                kind: filament_core::ChannelKind::Text,
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
            default_join_role_id: None,
            members: HashMap::new(),
            banned_members: std::collections::HashSet::new(),
            channels: HashMap::new(),
        };
        guild.channels.insert(
            String::from("other"),
            ChannelRecord {
                name: String::from("other"),
                kind: filament_core::ChannelKind::Text,
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
