use std::collections::HashMap;

use filament_core::UserId;

use crate::server::{core::AttachmentRecord, errors::AuthFailure};

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use filament_core::UserId;

    use super::bind_message_attachments_in_memory;
    use crate::server::{core::AttachmentRecord, errors::AuthFailure};

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
    fn binds_each_attachment_to_message_when_all_constraints_match() {
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
    fn rejects_when_attachment_is_missing() {
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
    }

    #[test]
    fn rejects_when_attachment_owner_or_binding_is_invalid() {
        let owner_id = UserId::new();
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
}
