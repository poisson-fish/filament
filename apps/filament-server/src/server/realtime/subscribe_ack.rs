use tokio::sync::mpsc;

pub(crate) enum SubscribeAckEnqueueResult {
    Enqueued,
    Rejected,
}

pub(crate) fn try_enqueue_subscribed_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
) -> SubscribeAckEnqueueResult {
    if outbound_tx.try_send(payload).is_ok() {
        SubscribeAckEnqueueResult::Enqueued
    } else {
        SubscribeAckEnqueueResult::Rejected
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::{try_enqueue_subscribed_event, SubscribeAckEnqueueResult};

    #[test]
    fn returns_enqueued_when_sender_has_capacity() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_subscribed_event(&tx, String::from("payload"));

        assert!(matches!(result, SubscribeAckEnqueueResult::Enqueued));
    }

    #[test]
    fn returns_rejected_when_sender_is_full_or_closed() {
        let (tx, rx) = mpsc::channel::<String>(1);
        tx.try_send(String::from("first"))
            .expect("first send should fill queue");

        let full_result = try_enqueue_subscribed_event(&tx, String::from("second"));
        assert!(matches!(full_result, SubscribeAckEnqueueResult::Rejected));

        drop(rx);
        let closed_result = try_enqueue_subscribed_event(&tx, String::from("third"));
        assert!(matches!(closed_result, SubscribeAckEnqueueResult::Rejected));
    }
}
