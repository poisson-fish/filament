use std::{collections::HashMap, fmt::Write as _};

use super::core::{MetricsState, METRICS_STATE};

pub(crate) const GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND: &str = "oversized_outbound";
pub(crate) const GATEWAY_DROP_REASON_SERIALIZE_ERROR: &str = "serialize_error";

pub(crate) fn metrics_state() -> &'static MetricsState {
    METRICS_STATE.get_or_init(MetricsState::default)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn render_metrics() -> String {
    let auth_failures = metrics_state()
        .auth_failures
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let rate_limit_hits = metrics_state()
        .rate_limit_hits
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let ws_disconnects = metrics_state()
        .ws_disconnects
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let gateway_events_emitted = metrics_state()
        .gateway_events_emitted
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let gateway_events_dropped = metrics_state()
        .gateway_events_dropped
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let gateway_events_unknown_received = metrics_state()
        .gateway_events_unknown_received
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let gateway_events_parse_rejected = metrics_state()
        .gateway_events_parse_rejected
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());
    let voice_sync_repairs = metrics_state()
        .voice_sync_repairs
        .lock()
        .map_or_else(|_| HashMap::new(), |guard| guard.clone());

    let mut output = String::new();
    output
        .push_str("# HELP filament_auth_failures_total Count of auth-related failures by reason\n");
    output.push_str("# TYPE filament_auth_failures_total counter\n");
    let mut auth_entries: Vec<_> = auth_failures.into_iter().collect();
    auth_entries.sort_by_key(|(reason, _)| *reason);
    for (reason, value) in auth_entries {
        let _ = writeln!(
            output,
            "filament_auth_failures_total{{reason=\"{reason}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_rate_limit_hits_total Count of rate-limit rejections by surface\n",
    );
    output.push_str("# TYPE filament_rate_limit_hits_total counter\n");
    let mut rate_entries: Vec<_> = rate_limit_hits.into_iter().collect();
    rate_entries.sort_by_key(|((surface, reason), _)| (*surface, *reason));
    for ((surface, reason), value) in rate_entries {
        let _ = writeln!(
            output,
            "filament_rate_limit_hits_total{{surface=\"{surface}\",reason=\"{reason}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_ws_disconnects_total Count of websocket disconnect events by reason\n",
    );
    output.push_str("# TYPE filament_ws_disconnects_total counter\n");
    let mut ws_entries: Vec<_> = ws_disconnects.into_iter().collect();
    ws_entries.sort_by_key(|(reason, _)| *reason);
    for (reason, value) in ws_entries {
        let _ = writeln!(
            output,
            "filament_ws_disconnects_total{{reason=\"{reason}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_gateway_events_emitted_total Count of emitted gateway events by scope and type\n",
    );
    output.push_str("# TYPE filament_gateway_events_emitted_total counter\n");
    let mut emitted_entries: Vec<_> = gateway_events_emitted.into_iter().collect();
    emitted_entries.sort_by(|((a_scope, a_event), _), ((b_scope, b_event), _)| {
        a_scope.cmp(b_scope).then(a_event.cmp(b_event))
    });
    for ((scope, event_type), value) in emitted_entries {
        let _ = writeln!(
            output,
            "filament_gateway_events_emitted_total{{scope=\"{scope}\",event_type=\"{event_type}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_gateway_events_dropped_total Count of dropped gateway events by scope, type, and reason\n",
    );
    output.push_str("# TYPE filament_gateway_events_dropped_total counter\n");
    let mut dropped_entries: Vec<_> = gateway_events_dropped.into_iter().collect();
    dropped_entries.sort_by(
        |((a_scope, a_event, a_reason), _), ((b_scope, b_event, b_reason), _)| {
            a_scope
                .cmp(b_scope)
                .then(a_event.cmp(b_event))
                .then(a_reason.cmp(b_reason))
        },
    );
    for ((scope, event_type, reason), value) in dropped_entries {
        let _ = writeln!(
            output,
            "filament_gateway_events_dropped_total{{scope=\"{scope}\",event_type=\"{event_type}\",reason=\"{reason}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_gateway_events_unknown_received_total Count of unknown gateway events received by scope and event type\n",
    );
    output.push_str("# TYPE filament_gateway_events_unknown_received_total counter\n");
    let mut unknown_entries: Vec<_> = gateway_events_unknown_received.into_iter().collect();
    unknown_entries.sort_by(|((a_scope, a_event), _), ((b_scope, b_event), _)| {
        a_scope.cmp(b_scope).then(a_event.cmp(b_event))
    });
    for ((scope, event_type), value) in unknown_entries {
        let _ = writeln!(
            output,
            "filament_gateway_events_unknown_received_total{{scope=\"{scope}\",event_type=\"{event_type}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_gateway_events_parse_rejected_total Count of gateway events rejected during parsing by scope and reason\n",
    );
    output.push_str("# TYPE filament_gateway_events_parse_rejected_total counter\n");
    let mut parse_rejected_entries: Vec<_> = gateway_events_parse_rejected.into_iter().collect();
    parse_rejected_entries.sort_by(|((a_scope, a_reason), _), ((b_scope, b_reason), _)| {
        a_scope.cmp(b_scope).then(a_reason.cmp(b_reason))
    });
    for ((scope, reason), value) in parse_rejected_entries {
        let _ = writeln!(
            output,
            "filament_gateway_events_parse_rejected_total{{scope=\"{scope}\",reason=\"{reason}\"}} {value}"
        );
    }

    output.push_str(
        "# HELP filament_voice_sync_repairs_total Count of voice drift-repair snapshots emitted by reason\n",
    );
    output.push_str("# TYPE filament_voice_sync_repairs_total counter\n");
    let mut voice_repair_entries: Vec<_> = voice_sync_repairs.into_iter().collect();
    voice_repair_entries.sort_by_key(|(reason, _)| reason.clone());
    for (reason, value) in voice_repair_entries {
        let _ = writeln!(
            output,
            "filament_voice_sync_repairs_total{{reason=\"{reason}\"}} {value}"
        );
    }

    output
}

pub(crate) fn record_auth_failure(reason: &'static str) {
    if let Ok(mut counters) = metrics_state().auth_failures.lock() {
        let entry = counters.entry(reason).or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_rate_limit_hit(surface: &'static str, reason: &'static str) {
    if let Ok(mut counters) = metrics_state().rate_limit_hits.lock() {
        let entry = counters.entry((surface, reason)).or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_ws_disconnect(reason: &'static str) {
    if let Ok(mut counters) = metrics_state().ws_disconnects.lock() {
        let entry = counters.entry(reason).or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_gateway_event_emitted(scope: &'static str, event_type: &str) {
    if let Ok(mut counters) = metrics_state().gateway_events_emitted.lock() {
        let entry = counters
            .entry((scope.to_owned(), event_type.to_owned()))
            .or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_gateway_event_dropped(
    scope: &'static str,
    event_type: &str,
    reason: &'static str,
) {
    if let Ok(mut counters) = metrics_state().gateway_events_dropped.lock() {
        let entry = counters
            .entry((scope.to_owned(), event_type.to_owned(), reason.to_owned()))
            .or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_gateway_event_serialize_error(scope: &'static str, event_type: &str) {
    record_gateway_event_dropped(scope, event_type, GATEWAY_DROP_REASON_SERIALIZE_ERROR);
}

pub(crate) fn record_gateway_event_oversized_outbound(scope: &'static str, event_type: &str) {
    record_gateway_event_dropped(scope, event_type, GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND);
}

pub(crate) fn record_gateway_event_unknown_received(scope: &'static str, event_type: &str) {
    if let Ok(mut counters) = metrics_state().gateway_events_unknown_received.lock() {
        let entry = counters
            .entry((scope.to_owned(), event_type.to_owned()))
            .or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_gateway_event_parse_rejected(scope: &'static str, reason: &'static str) {
    if let Ok(mut counters) = metrics_state().gateway_events_parse_rejected.lock() {
        let entry = counters
            .entry((scope.to_owned(), reason.to_owned()))
            .or_insert(0);
        *entry += 1;
    }
}

pub(crate) fn record_voice_sync_repair(reason: &'static str) {
    if let Ok(mut counters) = metrics_state().voice_sync_repairs.lock() {
        let entry = counters.entry(reason.to_owned()).or_insert(0);
        *entry += 1;
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{
        metrics_state, record_gateway_event_oversized_outbound,
        record_gateway_event_serialize_error, GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND,
        GATEWAY_DROP_REASON_SERIALIZE_ERROR,
    };

    #[test]
    fn records_serialize_error_with_canonical_reason_label() {
        let event_type = format!("serialize_test_{}", Uuid::new_v4());
        record_gateway_event_serialize_error("connection", &event_type);

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        let key = (
            String::from("connection"),
            event_type,
            String::from(GATEWAY_DROP_REASON_SERIALIZE_ERROR),
        );
        assert_eq!(dropped.get(&key).copied(), Some(1));
    }

    #[test]
    fn records_oversized_outbound_with_canonical_reason_label() {
        let event_type = format!("oversized_test_{}", Uuid::new_v4());
        record_gateway_event_oversized_outbound("guild", &event_type);

        let dropped = metrics_state()
            .gateway_events_dropped
            .lock()
            .expect("gateway dropped metrics mutex should not be poisoned");
        let key = (
            String::from("guild"),
            event_type,
            String::from(GATEWAY_DROP_REASON_OVERSIZED_OUTBOUND),
        );
        assert_eq!(dropped.get(&key).copied(), Some(1));
    }
}
