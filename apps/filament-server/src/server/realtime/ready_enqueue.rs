use tokio::sync::mpsc;

pub(crate) enum ReadyEnqueueResult {
    Enqueued,
    Closed,
    Full,
    Oversized,
}

pub(crate) fn try_enqueue_ready_event(
    outbound_tx: &mpsc::Sender<String>,
    payload: String,
    max_gateway_event_bytes: usize,
) -> ReadyEnqueueResult {
    if payload.len() > max_gateway_event_bytes {
        return ReadyEnqueueResult::Oversized;
    }

    match outbound_tx.try_send(payload) {
        Ok(()) => ReadyEnqueueResult::Enqueued,
        Err(mpsc::error::TrySendError::Closed(_)) => ReadyEnqueueResult::Closed,
        Err(mpsc::error::TrySendError::Full(_)) => ReadyEnqueueResult::Full,
    }
}

pub(crate) fn ready_error_reason(result: &ReadyEnqueueResult) -> Option<&'static str> {
    match result {
        ReadyEnqueueResult::Enqueued => None,
        ReadyEnqueueResult::Full => Some("outbound_queue_full"),
        ReadyEnqueueResult::Closed => Some("outbound_queue_closed"),
        ReadyEnqueueResult::Oversized => Some("outbound_payload_too_large"),
    }
}

pub(crate) fn ready_drop_metric_reason(result: &ReadyEnqueueResult) -> Option<&'static str> {
    match result {
        ReadyEnqueueResult::Enqueued => None,
        ReadyEnqueueResult::Full => Some("full_queue"),
        ReadyEnqueueResult::Closed => Some("closed"),
        ReadyEnqueueResult::Oversized => Some("oversized_outbound"),
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::{
        ready_drop_metric_reason, ready_error_reason, try_enqueue_ready_event, ReadyEnqueueResult,
    };

    #[test]
    fn returns_enqueued_when_sender_has_capacity() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Enqueued));
    }

    #[test]
    fn returns_full_when_sender_is_full() {
        let (tx, _rx) = mpsc::channel::<String>(1);
        tx.try_send(String::from("first"))
            .expect("first send should fill queue");

        let result = try_enqueue_ready_event(&tx, String::from("second"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Full));
    }

    #[test]
    fn returns_closed_when_sender_is_closed() {
        let (tx, rx) = mpsc::channel::<String>(1);
        drop(rx);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 1024);

        assert!(matches!(result, ReadyEnqueueResult::Closed));
    }

    #[test]
    fn returns_oversized_when_payload_exceeds_limit() {
        let (tx, _rx) = mpsc::channel::<String>(1);

        let result = try_enqueue_ready_event(&tx, String::from("payload"), 3);

        assert!(matches!(result, ReadyEnqueueResult::Oversized));
    }

    #[test]
    fn ready_error_reason_maps_all_rejections() {
        assert_eq!(ready_error_reason(&ReadyEnqueueResult::Enqueued), None);
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Full),
            Some("outbound_queue_full")
        );
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Closed),
            Some("outbound_queue_closed")
        );
        assert_eq!(
            ready_error_reason(&ReadyEnqueueResult::Oversized),
            Some("outbound_payload_too_large")
        );
    }

    #[test]
    fn ready_drop_metric_reason_maps_all_rejections() {
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Enqueued),
            None
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Full),
            Some("full_queue")
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Closed),
            Some("closed")
        );
        assert_eq!(
            ready_drop_metric_reason(&ReadyEnqueueResult::Oversized),
            Some("oversized_outbound")
        );
    }
}
