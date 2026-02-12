fn metrics_state() -> &'static MetricsState {
    METRICS_STATE.get_or_init(MetricsState::default)
}

fn render_metrics() -> String {
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

    output
}

fn record_auth_failure(reason: &'static str) {
    if let Ok(mut counters) = metrics_state().auth_failures.lock() {
        let entry = counters.entry(reason).or_insert(0);
        *entry += 1;
    }
}

fn record_rate_limit_hit(surface: &'static str, reason: &'static str) {
    if let Ok(mut counters) = metrics_state().rate_limit_hits.lock() {
        let entry = counters.entry((surface, reason)).or_insert(0);
        *entry += 1;
    }
}

fn record_ws_disconnect(reason: &'static str) {
    if let Ok(mut counters) = metrics_state().ws_disconnects.lock() {
        let entry = counters.entry(reason).or_insert(0);
        *entry += 1;
    }
}

