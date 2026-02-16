use super::ingress_command::GatewayIngressCommandParseError;

pub(crate) enum IngressCommandParseClassification<'a> {
    ParseRejected(&'static str),
    UnknownEventType(&'a str),
}

pub(crate) fn classify_ingress_command_parse_error(
    error: &GatewayIngressCommandParseError,
) -> IngressCommandParseClassification<'_> {
    match error {
        GatewayIngressCommandParseError::InvalidSubscribePayload => {
            IngressCommandParseClassification::ParseRejected("invalid_subscribe_payload")
        }
        GatewayIngressCommandParseError::InvalidMessageCreatePayload => {
            IngressCommandParseClassification::ParseRejected("invalid_message_create_payload")
        }
        GatewayIngressCommandParseError::UnknownEventType(event_type) => {
            IngressCommandParseClassification::UnknownEventType(event_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_ingress_command_parse_error, IngressCommandParseClassification};
    use crate::server::realtime::ingress_command::GatewayIngressCommandParseError;

    #[test]
    fn classifies_invalid_subscribe_payload_as_parse_rejected() {
        let classification = classify_ingress_command_parse_error(
            &GatewayIngressCommandParseError::InvalidSubscribePayload,
        );

        assert!(matches!(
            classification,
            IngressCommandParseClassification::ParseRejected("invalid_subscribe_payload")
        ));
    }

    #[test]
    fn classifies_invalid_message_create_payload_as_parse_rejected() {
        let classification = classify_ingress_command_parse_error(
            &GatewayIngressCommandParseError::InvalidMessageCreatePayload,
        );

        assert!(matches!(
            classification,
            IngressCommandParseClassification::ParseRejected("invalid_message_create_payload")
        ));
    }

    #[test]
    fn classifies_unknown_event_type_as_unknown_event() {
        let error =
            GatewayIngressCommandParseError::UnknownEventType(String::from("presence_sync"));
        let classification = classify_ingress_command_parse_error(&error);

        assert!(matches!(
            classification,
            IngressCommandParseClassification::UnknownEventType("presence_sync")
        ));
    }
}
