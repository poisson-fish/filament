use serde::Serialize;

use crate::server::auth::outbound_event;

pub(crate) struct GatewayEvent {
    pub(crate) event_type: &'static str,
    pub(crate) payload: String,
}

pub(super) fn build_event<T: Serialize>(event_type: &'static str, payload: T) -> GatewayEvent {
    GatewayEvent {
        event_type,
        payload: outbound_event(event_type, payload),
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::Value;

    use super::{build_event, GatewayEvent};

    #[derive(Serialize)]
    struct EnvelopeTestPayload<'a> {
        value: &'a str,
    }

    fn parse_envelope(event: &GatewayEvent) -> Value {
        serde_json::from_str(&event.payload).expect("event payload should be valid json")
    }

    #[test]
    fn build_event_wraps_typed_payload_in_gateway_envelope() {
        let event = build_event("test_event", EnvelopeTestPayload { value: "ok" });
        let envelope = parse_envelope(&event);
        assert_eq!(envelope["v"], Value::from(1));
        assert_eq!(envelope["t"], Value::from("test_event"));
        assert_eq!(envelope["d"]["value"], Value::from("ok"));
    }
}