use filament_core::{DeviceId, UserId};
use filament_protocol::{DeviceListUpdateEvent, KeyPackageLowEvent};

use super::{envelope::try_build_event, GatewayEvent};

pub(crate) const DEVICE_LIST_UPDATE_EVENT: &str = "device_list_update";
pub(crate) const KEYPACKAGE_LOW_EVENT: &str = "keypackage_low";

pub(crate) fn try_device_list_update(
    user_id: UserId,
    device_count: u32,
    created_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        DEVICE_LIST_UPDATE_EVENT,
        DeviceListUpdateEvent {
            user_id: user_id.to_string(),
            device_count,
            created_at_unix,
        },
    )
}

pub(crate) fn try_keypackage_low(
    device_id: DeviceId,
    remaining_count: u32,
    water_mark: u32,
    created_at_unix: i64,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(
        KEYPACKAGE_LOW_EVENT,
        KeyPackageLowEvent {
            device_id: device_id.to_string(),
            remaining_count,
            water_mark,
            created_at_unix,
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn parse_payload(event: &GatewayEvent) -> Value {
        let envelope: Value =
            serde_json::from_str(&event.payload).expect("gateway event should be valid JSON");
        assert_eq!(envelope["v"], Value::from(1));
        assert_eq!(envelope["t"], Value::from(event.event_type));
        envelope["d"].clone()
    }

    #[test]
    fn device_list_update_uses_typed_contract() {
        let user_id = UserId::new();
        let payload =
            parse_payload(&try_device_list_update(user_id, 2, 10).expect("event should serialize"));
        assert_eq!(payload["user_id"], Value::from(user_id.to_string()));
        assert_eq!(payload["device_count"], Value::from(2));
        assert_eq!(payload["created_at_unix"], Value::from(10));
    }

    #[test]
    fn keypackage_low_uses_typed_contract() {
        let device_id = DeviceId::new();
        let payload = parse_payload(
            &try_keypackage_low(device_id, 4, 10, 11).expect("event should serialize"),
        );
        assert_eq!(payload["device_id"], Value::from(device_id.to_string()));
        assert_eq!(payload["remaining_count"], Value::from(4));
        assert_eq!(payload["water_mark"], Value::from(10));
        assert_eq!(payload["created_at_unix"], Value::from(11));
    }
}
