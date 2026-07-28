use filament_core::{DeviceId, UserId};
use filament_protocol::{
    DeviceListUpdateEvent, KeyPackageLowEvent, MlsCommitEvent, MlsMembershipChangeEvent,
    MlsMessageEvent, MlsProposalEvent, MlsWelcomeEvent,
};

use super::{envelope::try_build_event, GatewayEvent};

pub(crate) const DEVICE_LIST_UPDATE_EVENT: &str = "device_list_update";
pub(crate) const KEYPACKAGE_LOW_EVENT: &str = "keypackage_low";
pub(crate) const MLS_COMMIT_EVENT: &str = "mls_commit";
pub(crate) const MLS_MESSAGE_EVENT: &str = "mls_message";
pub(crate) const MLS_MEMBERSHIP_CHANGE_EVENT: &str = "mls_membership_change";
pub(crate) const MLS_PROPOSAL_EVENT: &str = "mls_proposal";
pub(crate) const MLS_WELCOME_EVENT: &str = "mls_welcome";

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

pub(crate) fn try_mls_commit(payload: MlsCommitEvent) -> anyhow::Result<GatewayEvent> {
    try_build_event(MLS_COMMIT_EVENT, payload)
}

pub(crate) fn try_mls_message(payload: MlsMessageEvent) -> anyhow::Result<GatewayEvent> {
    try_build_event(MLS_MESSAGE_EVENT, payload)
}

pub(crate) fn try_mls_membership_change(
    payload: MlsMembershipChangeEvent,
) -> anyhow::Result<GatewayEvent> {
    try_build_event(MLS_MEMBERSHIP_CHANGE_EVENT, payload)
}

pub(crate) fn try_mls_proposal(payload: MlsProposalEvent) -> anyhow::Result<GatewayEvent> {
    try_build_event(MLS_PROPOSAL_EVENT, payload)
}

pub(crate) fn try_mls_welcome(payload: MlsWelcomeEvent) -> anyhow::Result<GatewayEvent> {
    try_build_event(MLS_WELCOME_EVENT, payload)
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
        for device_count in [0, 2] {
            let payload = parse_payload(
                &try_device_list_update(user_id, device_count, 10).expect("event should serialize"),
            );
            assert_eq!(payload["user_id"], Value::from(user_id.to_string()));
            assert_eq!(payload["device_count"], Value::from(device_count));
            assert_eq!(payload["created_at_unix"], Value::from(10));
        }
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

    #[test]
    fn mls_transport_events_use_typed_contracts() {
        let commit = parse_payload(
            &try_mls_commit(MlsCommitEvent {
                group_id: String::from("g"),
                conversation_id: String::from("c"),
                epoch: 2,
                prior_epoch: 1,
                committer_device_id: String::from("d"),
                created_at_unix: 10,
            })
            .expect("event should serialize"),
        );
        assert_eq!(commit["epoch"], Value::from(2));

        let message = parse_payload(
            &try_mls_message(MlsMessageEvent {
                group_id: String::from("g"),
                conversation_id: String::from("c"),
                message_id: String::from("m"),
                epoch: 2,
                suite_id: 3,
                sender_device_id: String::from("d"),
                created_at_unix: 11,
            })
            .expect("event should serialize"),
        );
        assert_eq!(message["suite_id"], Value::from(3));

        let welcome = parse_payload(
            &try_mls_welcome(MlsWelcomeEvent {
                group_id: String::from("g"),
                conversation_id: String::from("c"),
                epoch: 2,
                suite_id: 3,
                created_at_unix: 12,
            })
            .expect("event should serialize"),
        );
        assert_eq!(welcome["created_at_unix"], Value::from(12));

        let proposal = parse_payload(
            &try_mls_proposal(MlsProposalEvent {
                group_id: String::from("g"),
                conversation_id: String::from("c"),
                proposal_id: String::from("p"),
                epoch: 2,
                proposer_device_id: Some(String::from("d")),
                external_sender_index: None,
                reconciliation_deadline_unix: None,
                created_at_unix: 13,
            })
            .expect("event should serialize"),
        );
        assert_eq!(proposal["proposal_id"], Value::from("p"));
    }
}
