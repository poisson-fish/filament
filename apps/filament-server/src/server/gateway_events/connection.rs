use filament_core::UserId;
use serde::Serialize;

use super::{envelope::build_event, GatewayEvent};

pub(crate) const READY_EVENT: &str = "ready";
pub(crate) const SUBSCRIBED_EVENT: &str = "subscribed";

#[derive(Serialize)]
struct ReadyPayload {
    user_id: String,
}

#[derive(Serialize)]
struct SubscribedPayload<'a> {
    guild_id: &'a str,
    channel_id: &'a str,
}

pub(crate) fn ready(user_id: UserId) -> GatewayEvent {
    build_event(
        READY_EVENT,
        ReadyPayload {
            user_id: user_id.to_string(),
        },
    )
}

pub(crate) fn subscribed(guild_id: &str, channel_id: &str) -> GatewayEvent {
    build_event(
        SUBSCRIBED_EVENT,
        SubscribedPayload {
            guild_id,
            channel_id,
        },
    )
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
    fn ready_event_contains_authenticated_user_id() {
        let user_id = UserId::new();
            let payload = parse_payload(&ready(user_id));
        assert_eq!(payload["user_id"], Value::from(user_id.to_string()));
    }

    #[test]
    fn subscribed_event_contains_guild_and_channel_scope() {
            let payload = parse_payload(&subscribed("guild-1", "channel-1"));
        assert_eq!(payload["guild_id"], Value::from("guild-1"));
        assert_eq!(payload["channel_id"], Value::from("channel-1"));
    }
}
