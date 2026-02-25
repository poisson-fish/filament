use tokio::sync::mpsc;

pub(crate) enum SubscribeAckEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

pub(crate) fn try_enqueue_subscribed_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> SubscribeAckEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return SubscribeAckEnqueueResult::Oversized;
    }

    match outbound_tx.try_send(payload) {
        Ok(()) => SubscribeAckEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => SubscribeAckEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => SubscribeAckEnqueueResult::Full,
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::{try_enqueue_subscribed_event, SubscribeAckEnqueueResult};

    #[test]
    fn returns_enqueued_when_sender_has_capacity() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_subscribed_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, SubscribeAckEnqueueResult::Enqueued));
    }

    #[test]
    fn returns_full_when_sender_is_full() {
        let (tx, rx) = mpsc::channel::<String>(1);
        tx.try_send(String::from("first"))
            .expect("first send should fill queue");

        let full_result = try_enqueue_subscribed_event(&tx, String::from("second"), 1024);
        assert!(matches!(full_result, SubscribeAckEnqueueResult::Full));

        drop(rx);
    }

    #[test]
    fn returns_closed_when_sender_is_closed() {
        let (tx, rx) = mpsc::channel::<String>(1);
        drop(rx);
        let closed_result = try_enqueue_subscribed_event(&tx, String::from("third"), 1024);
        assert!(matches!(closed_result, SubscribeAckEnqueueResult::Closed));
    }

    #[test]
    fn returns_oversized_when_payload_exceeds_limit() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_subscribed_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, SubscribeAckEnqueueResult::Oversized));
    }
}
