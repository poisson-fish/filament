export const MAX_DIAGNOSTICS_EVENT_COUNT = 1_000_000;

export const DIAGNOSTICS_EVENT_TYPES = [
  "session_refresh_succeeded",
  "session_refresh_failed",
  "health_check_succeeded",
  "health_check_failed",
  "echo_succeeded",
  "echo_failed",
  "logout_requested",
  "gateway_connected",
  "gateway_disconnected",
] as const;

export type DiagnosticsEventType = (typeof DIAGNOSTICS_EVENT_TYPES)[number];

export type DiagnosticsEventCounts = Record<DiagnosticsEventType, number>;

export function createInitialDiagnosticsEventCounts(): DiagnosticsEventCounts {
  return {
    session_refresh_succeeded: 0,
    session_refresh_failed: 0,
    health_check_succeeded: 0,
    health_check_failed: 0,
    echo_succeeded: 0,
    echo_failed: 0,
    logout_requested: 0,
    gateway_connected: 0,
    gateway_disconnected: 0,
  };
}

export function incrementDiagnosticsEventCount(
  counts: DiagnosticsEventCounts,
  eventType: DiagnosticsEventType,
): DiagnosticsEventCounts {
  const nextValue = Math.min((counts[eventType] ?? 0) + 1, MAX_DIAGNOSTICS_EVENT_COUNT);
  return {
    ...counts,
    [eventType]: nextValue,
  };
}
