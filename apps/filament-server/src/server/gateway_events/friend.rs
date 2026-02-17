use filament_core::UserId;
use serde::Serialize;

use super::{envelope::build_event, GatewayEvent};

pub(crate) const FRIEND_REQUEST_CREATE_EVENT: &str = "friend_request_create";
pub(crate) const FRIEND_REQUEST_UPDATE_EVENT: &str = "friend_request_update";
pub(crate) const FRIEND_REQUEST_DELETE_EVENT: &str = "friend_request_delete";
pub(crate) const FRIEND_REMOVE_EVENT: &str = "friend_remove";

#[derive(Serialize)]
struct FriendRequestCreatePayload<'a> {
    request_id: &'a str,
    sender_user_id: &'a str,
    sender_username: &'a str,
    recipient_user_id: &'a str,
    recipient_username: &'a str,
    created_at_unix: i64,
}

#[derive(Serialize)]
struct FriendRequestUpdatePayload<'a> {
    request_id: &'a str,
    state: &'static str,
    user_id: &'a str,
    friend_user_id: &'a str,
    friend_username: &'a str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRequestDeletePayload<'a> {
    request_id: &'a str,
    deleted_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[derive(Serialize)]
struct FriendRemovePayload<'a> {
    user_id: &'a str,
    friend_user_id: &'a str,
    removed_at_unix: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_user_id: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_create(
    request_id: &str,
    sender_user_id: &str,
    sender_username: &str,
    recipient_user_id: &str,
    recipient_username: &str,
    created_at_unix: i64,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_CREATE_EVENT,
        FriendRequestCreatePayload {
            request_id,
            sender_user_id,
            sender_username,
            recipient_user_id,
            recipient_username,
            created_at_unix,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn friend_request_update(
    request_id: &str,
    user_id: &str,
    friend_user_id: &str,
    friend_username: &str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_UPDATE_EVENT,
        FriendRequestUpdatePayload {
            request_id,
            state: "accepted",
            user_id,
            friend_user_id,
            friend_username,
            friendship_created_at_unix,
            updated_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_request_delete(
    request_id: &str,
    deleted_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REQUEST_DELETE_EVENT,
        FriendRequestDeletePayload {
            request_id,
            deleted_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

pub(crate) fn friend_remove(
    user_id: &str,
    friend_user_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    build_event(
        FRIEND_REMOVE_EVENT,
        FriendRemovePayload {
            user_id,
            friend_user_id,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

#[cfg(test)]
mod tests {
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
    fn friend_request_create_event_emits_recipient_username() {
        let payload = parse_payload(friend_request_create(
            "req-1", "user-1", "alice", "user-2", "bob", 77,
        ));
        assert_eq!(payload["recipient_username"], Value::from("bob"));
    }

    #[test]
    fn friend_request_update_event_emits_accepted_state() {
        let payload = parse_payload(friend_request_update(
            "req-1", "user-1", "user-2", "bob", 88, 89, None,
        ));
        assert_eq!(payload["state"], Value::from("accepted"));
    }

    #[test]
    fn friend_remove_event_emits_removed_timestamp() {
        let payload = parse_payload(friend_remove("user-1", "user-2", 99, None));
        assert_eq!(payload["removed_at_unix"], Value::from(99));
    }
}
