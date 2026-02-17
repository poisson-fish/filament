use axum::extract::ws::Message;

pub(crate) enum GatewayIngressMessageDecode {
    Payload(Vec<u8>),
    Continue,
    Disconnect(&'static str),
}

pub(crate) fn decode_gateway_ingress_message(
    message: Message,
    max_gateway_event_bytes: usize,
) -> GatewayIngressMessageDecode {
    match message {
        Message::Text(text) => {
            if text.len() > max_gateway_event_bytes {
                return GatewayIngressMessageDecode::Disconnect("event_too_large");
            }
            GatewayIngressMessageDecode::Payload(text.as_bytes().to_vec())
        }
        Message::Binary(bytes) => {
            if bytes.len() > max_gateway_event_bytes {
                return GatewayIngressMessageDecode::Disconnect("event_too_large");
            }
            GatewayIngressMessageDecode::Payload(bytes.to_vec())
        }
        Message::Close(_) => GatewayIngressMessageDecode::Disconnect("client_close"),
        Message::Ping(_) | Message::Pong(_) => GatewayIngressMessageDecode::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_gateway_ingress_message, GatewayIngressMessageDecode};
    use axum::extract::ws::Message;

    #[test]
    fn decodes_text_payload_when_within_cap() {
        let message = Message::Text("{\"v\":1,\"t\":\"subscribe\",\"d\":{}}".into());

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Payload(payload) => {
                assert_eq!(payload, b"{\"v\":1,\"t\":\"subscribe\",\"d\":{}}".to_vec());
            }
            GatewayIngressMessageDecode::Continue => panic!("expected payload"),
            GatewayIngressMessageDecode::Disconnect(reason) => {
                panic!("unexpected disconnect: {reason}")
            }
        }
    }

    #[test]
    fn rejects_oversized_binary_payload() {
        let message = Message::Binary(vec![1_u8, 2_u8, 3_u8].into());

        match decode_gateway_ingress_message(message, 2) {
            GatewayIngressMessageDecode::Disconnect(reason) => {
                assert_eq!(reason, "event_too_large");
            }
            GatewayIngressMessageDecode::Payload(_) | GatewayIngressMessageDecode::Continue => {
                panic!("expected disconnect")
            }
        }
    }

    #[test]
    fn maps_close_to_client_close_disconnect() {
        let message = Message::Close(None);

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Disconnect(reason) => {
                assert_eq!(reason, "client_close");
            }
            GatewayIngressMessageDecode::Payload(_) | GatewayIngressMessageDecode::Continue => {
                panic!("expected disconnect")
            }
        }
    }

    #[test]
    fn ignores_ping_messages() {
        let message = Message::Ping(vec![1_u8].into());

        match decode_gateway_ingress_message(message, 256) {
            GatewayIngressMessageDecode::Continue => {}
            GatewayIngressMessageDecode::Payload(_) => panic!("expected continue"),
            GatewayIngressMessageDecode::Disconnect(reason) => {
                panic!("unexpected disconnect: {reason}")
            }
        }
    }
}
