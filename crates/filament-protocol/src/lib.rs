#![forbid(unsafe_code)]

mod events;

use serde::{Deserialize, Serialize};

pub use events::{
    gateway_event_manifest, parse_gateway_event_manifest, GatewayEventLifecycle,
    GatewayEventManifest, GatewayEventManifestEntry, GatewayEventManifestError, GatewayEventScope,
};

/// Current gateway envelope version.
pub const PROTOCOL_VERSION: u16 = 1;
/// Maximum allowed gateway payload bytes.
pub const MAX_EVENT_BYTES: usize = 64 * 1024;

/// Versioned gateway envelope. All events use `{ v, t, d }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub v: u16,
    pub t: EventType,
    pub d: T,
}

/// Event type identifier with a strict character allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventType(String);

impl EventType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventType {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_event_type(&value)?;
        Ok(Self(value))
    }
}

impl From<EventType> for String {
    fn from(value: EventType) -> Self {
        value.0
    }
}

/// Parse and validate an incoming envelope at the network boundary.
///
/// # Errors
/// Returns [`ProtocolError`] if the payload exceeds limits, is malformed JSON,
/// contains an unsupported version, or has an invalid event type.
pub fn parse_envelope(input: &[u8]) -> Result<Envelope<serde_json::Value>, ProtocolError> {
    if input.len() > MAX_EVENT_BYTES {
        return Err(ProtocolError::OversizedPayload {
            max: MAX_EVENT_BYTES,
            actual: input.len(),
        });
    }

    let envelope: Envelope<serde_json::Value> = serde_json::from_slice(input)?;
    if envelope.v != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion {
            expected: PROTOCOL_VERSION,
            actual: envelope.v,
        });
    }

    Ok(envelope)
}

pub(crate) fn validate_event_type(value: &str) -> Result<(), ProtocolError> {
    const MAX_LEN: usize = 64;

    if value.is_empty() || value.len() > MAX_LEN {
        return Err(ProtocolError::InvalidEventType);
    }

    if value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
    {
        return Ok(());
    }

    Err(ProtocolError::InvalidEventType)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("payload exceeds max size: max={max} bytes actual={actual} bytes")]
    OversizedPayload { max: usize, actual: usize },
    #[error("unsupported envelope version: expected={expected} actual={actual}")]
    UnsupportedVersion { expected: u16, actual: u16 },
    #[error("invalid event type")]
    InvalidEventType,
    #[error("invalid json payload")]
    InvalidJson,
}

impl From<serde_json::Error> for ProtocolError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidJson
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_envelope, EventType, ProtocolError, PROTOCOL_VERSION};

    #[test]
    fn event_type_accepts_valid_identifier() {
        let event_type = EventType::try_from(String::from("message_create")).unwrap();
        assert_eq!(event_type.as_str(), "message_create");
    }

    #[test]
    fn event_type_rejects_invalid_identifier() {
        let error = EventType::try_from(String::from("message-create")).unwrap_err();
        assert_eq!(error, ProtocolError::InvalidEventType);
    }

    #[test]
    fn parse_rejects_unsupported_version() {
        let payload = br#"{"v":99,"t":"ready","d":{}}"#;
        let error = parse_envelope(payload).unwrap_err();
        assert_eq!(
            error,
            ProtocolError::UnsupportedVersion {
                expected: PROTOCOL_VERSION,
                actual: 99,
            }
        );
    }

    #[test]
    fn parse_rejects_unknown_fields() {
        let payload = br#"{"v":1,"t":"ready","d":{},"extra":1}"#;
        let error = parse_envelope(payload).unwrap_err();
        assert_eq!(error, ProtocolError::InvalidJson);
    }

    #[test]
    fn parse_accepts_valid_payload() {
        let payload = br#"{"v":1,"t":"ready","d":{"session":"abc"}}"#;
        let envelope = parse_envelope(payload).unwrap();

        assert_eq!(envelope.v, 1);
        assert_eq!(envelope.t.as_str(), "ready");
        assert_eq!(envelope.d["session"], "abc");
    }
}
