use serde::Serialize;

use super::{envelope::build_event, GatewayEvent};

pub(crate) const PROFILE_UPDATE_EVENT: &str = "profile_update";
pub(crate) const PROFILE_AVATAR_UPDATE_EVENT: &str = "profile_avatar_update";

#[derive(Serialize)]
struct ProfileUpdatePayload<'a> {
    user_id: &'a str,
    updated_fields: ProfileUpdateFieldsPayload<'a>,
    updated_at_unix: i64,
}

#[derive(Serialize)]
struct ProfileUpdateFieldsPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    about_markdown_tokens: Option<&'a [filament_core::MarkdownToken]>,
}

#[derive(Serialize)]
struct ProfileAvatarUpdatePayload<'a> {
    user_id: &'a str,
    avatar_version: i64,
    updated_at_unix: i64,
}

pub(crate) fn profile_update(
    user_id: &str,
    username: Option<&str>,
    about_markdown: Option<&str>,
    about_markdown_tokens: Option<&[filament_core::MarkdownToken]>,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_UPDATE_EVENT,
        ProfileUpdatePayload {
            user_id,
            updated_fields: ProfileUpdateFieldsPayload {
                username,
                about_markdown,
                about_markdown_tokens,
            },
            updated_at_unix,
        },
    )
}

pub(crate) fn profile_avatar_update(
    user_id: &str,
    avatar_version: i64,
    updated_at_unix: i64,
) -> GatewayEvent {
    build_event(
        PROFILE_AVATAR_UPDATE_EVENT,
        ProfileAvatarUpdatePayload {
            user_id,
            avatar_version,
            updated_at_unix,
        },
    )
}

#[cfg(test)]
mod tests {
    use filament_core::MarkdownToken;
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be valid");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        value["d"].clone()
    }

    #[test]
    fn profile_update_event_emits_profile_fields() {
        let payload = parse_payload(profile_update(
            "user-1",
            Some("alice"),
            Some("about"),
            Some(&[MarkdownToken::Text {
                text: String::from("about"),
            }]),
            44,
        ));
        assert_eq!(payload["user_id"], Value::from("user-1"));
        assert_eq!(payload["updated_fields"]["username"], Value::from("alice"));
    }

    #[test]
    fn profile_avatar_update_event_emits_avatar_version() {
        let payload = parse_payload(profile_avatar_update("user-1", 3, 55));
        assert_eq!(payload["avatar_version"], Value::from(3));
        assert_eq!(payload["updated_at_unix"], Value::from(55));
    }
}
