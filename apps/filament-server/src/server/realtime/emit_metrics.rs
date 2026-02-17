use crate::server::metrics::record_gateway_event_emitted;

pub(crate) fn emit_gateway_delivery_metrics(
    scope: &'static str,
    event_type: &'static str,
    delivered: usize,
) -> usize {
    if delivered == 0 {
        return 0;
    }

    tracing::debug!(
        event = "gateway.event.emit",
        scope,
        event_type,
        delivered
    );
    for _ in 0..delivered {
        record_gateway_event_emitted(scope, event_type);
    }

    delivered
}

#[cfg(test)]
mod tests {
    use super::emit_gateway_delivery_metrics;

    #[test]
    fn returns_zero_when_nothing_delivered() {
        let emitted = emit_gateway_delivery_metrics("channel", "message_create", 0);

        assert_eq!(emitted, 0);
    }

    #[test]
    fn returns_delivered_count_when_events_emitted() {
        let emitted = emit_gateway_delivery_metrics("guild", "presence_update", 3);

        assert_eq!(emitted, 3);
    }
}
