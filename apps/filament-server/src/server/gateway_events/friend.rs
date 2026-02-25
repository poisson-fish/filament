use filament_core::UserId;
use serde::Serialize;

use super::{
    envelope::{build_event, try_build_event},
    GatewayEvent,
};

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
#[cfg(test)]
pub(crate) fn friend_request_create(
    request_id: &str,
    sender_user_id: &str,
    sender_username: &str,
    recipient_user_id: &str,
    recipient_username: &str,
    created_at_unix: i64,
) -> GatewayEvent {
    try_friend_request_create(
        request_id,
        sender_user_id,
        sender_username,
        recipient_user_id,
        recipient_username,
        created_at_unix,
    )
    .unwrap_or_else(|error| {
        panic!("failed to build outbound gateway event {FRIEND_REQUEST_CREATE_EVENT}: {error}")
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn try_friend_request_create(
    request_id: &str,
    sender_user_id: &str,
    sender_username: &str,
    recipient_user_id: &str,
    recipient_username: &str,
    created_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_friend_request_create_event(
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

fn try_build_friend_request_create_event(
    event_type: &'static str,
    payload: FriendRequestCreatePayload<'_>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(event_type, payload)
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(crate) fn friend_request_update(
    request_id: &str,
    user_id: &str,
    friend_user_id: &str,
    friend_username: &str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    try_friend_request_update(
        request_id,
        user_id,
        friend_user_id,
        friend_username,
        friendship_created_at_unix,
        updated_at_unix,
        actor_user_id,
    )
    .unwrap_or_else(|error| {
        panic!("failed to build outbound gateway event {FRIEND_REQUEST_UPDATE_EVENT}: {error}")
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn try_friend_request_update(
    request_id: &str,
    user_id: &str,
    friend_user_id: &str,
    friend_username: &str,
    friendship_created_at_unix: i64,
    updated_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> anyhow::Result<GatewayEvent> {
    try_build_friend_request_update(
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

fn try_build_friend_request_update(
    event_type: &'static str,
    payload: FriendRequestUpdatePayload<'_>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(event_type, payload)
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

#[cfg(test)]
pub(crate) fn friend_remove(
    user_id: &str,
    friend_user_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> GatewayEvent {
    try_friend_remove(user_id, friend_user_id, removed_at_unix, actor_user_id).unwrap_or_else(
        |error| panic!("failed to build outbound gateway event {FRIEND_REMOVE_EVENT}: {error}"),
    )
}

pub(crate) fn try_friend_remove(
    user_id: &str,
    friend_user_id: &str,
    removed_at_unix: i64,
    actor_user_id: Option<UserId>,
) -> anyhow::Result<GatewayEvent> {
    try_build_friend_remove_event(
        FRIEND_REMOVE_EVENT,
        FriendRemovePayload {
            user_id,
            friend_user_id,
            removed_at_unix,
            actor_user_id: actor_user_id.map(|id| id.to_string()),
        },
    )
}

fn try_build_friend_remove_event(
    event_type: &'static str,
    payload: FriendRemovePayload<'_>,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(event_type, payload)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: &GatewayEvent) -> Value {
        let value: Value =
            serde_json::from_str(&event.payload).expect("gateway event payload should be valid");
        assert_eq!(value["v"], Value::from(1));
        assert_eq!(value["t"], Value::from(event.event_type));
        value["d"].clone()
    }

    #[test]
    fn friend_request_create_event_emits_recipient_username() {
        let payload = parse_payload(&friend_request_create(
            "req-1", "user-1", "alice", "user-2", "bob", 77,
        ));
        assert_eq!(payload["recipient_username"], Value::from("bob"));
    }

    #[test]
    fn try_friend_request_create_emits_recipient_username() {
        let payload = parse_payload(
            &try_friend_request_create("req-1", "user-1", "alice", "user-2", "bob", 77)
                .expect("friend_request_create should serialize"),
        );
        assert_eq!(payload["recipient_username"], Value::from("bob"));
    }

    #[test]
    fn try_friend_request_create_rejects_invalid_event_type() {
        let Err(error) = try_build_friend_request_create_event(
            "friend request create",
            FriendRequestCreatePayload {
                request_id: "req-1",
                sender_user_id: "user-1",
                sender_username: "alice",
                recipient_user_id: "user-2",
                recipient_username: "bob",
                created_at_unix: 77,
            },
        ) else {
            panic!("invalid event type should fail");
        };
        assert!(
            error.to_string().contains("invalid outbound event type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn friend_request_update_event_emits_accepted_state() {
        let payload = parse_payload(
            &try_friend_request_update("req-1", "user-1", "user-2", "bob", 88, 89, None)
                .expect("friend_request_update should serialize"),
        );
        assert_eq!(payload["state"], Value::from("accepted"));
    }

    #[test]
    fn try_friend_request_update_rejects_invalid_event_type() {
        let Err(error) = try_build_friend_request_update(
            "friend request update",
            FriendRequestUpdatePayload {
                request_id: "req-1",
                state: "accepted",
                user_id: "user-1",
                friend_user_id: "user-2",
                friend_username: "bob",
                friendship_created_at_unix: 88,
                updated_at_unix: 89,
                actor_user_id: None,
            },
        ) else {
            panic!("invalid event type should fail");
        };
        assert!(
            error.to_string().contains("invalid outbound event type"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn friend_remove_event_emits_removed_timestamp() {
        let payload = parse_payload(
            &try_friend_remove("user-1", "user-2", 99, None)
                .expect("friend_remove should serialize"),
        );
        assert_eq!(payload["removed_at_unix"], Value::from(99));
    }

    #[test]
    fn try_friend_remove_rejects_invalid_event_type() {
        let Err(error) = try_build_friend_remove_event(
            "friend remove",
            FriendRemovePayload {
                user_id: "user-1",
                friend_user_id: "user-2",
                removed_at_unix: 99,
                actor_user_id: None,
            },
        ) else {
            panic!("invalid event type should fail");
        };
        assert!(
            error.to_string().contains("invalid outbound event type"),
            "unexpected error: {error}"
        );
    }
}
