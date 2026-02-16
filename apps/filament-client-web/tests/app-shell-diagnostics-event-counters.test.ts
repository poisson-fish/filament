import { describe, expect, it } from "vitest";
import {
  createInitialDiagnosticsEventCounts,
  incrementDiagnosticsEventCount,
  MAX_DIAGNOSTICS_EVENT_COUNT,
} from "../src/features/app-shell/state/diagnostics-event-counters";

describe("app shell diagnostics event counters", () => {
  it("initializes all counters to zero", () => {
    expect(createInitialDiagnosticsEventCounts()).toEqual({
      session_refresh_succeeded: 0,
      session_refresh_failed: 0,
      health_check_succeeded: 0,
      health_check_failed: 0,
      echo_succeeded: 0,
      echo_failed: 0,
      logout_requested: 0,
    });
  });

  it("increments selected counter with saturation", () => {
    const nearLimit = {
      ...createInitialDiagnosticsEventCounts(),
      echo_failed: MAX_DIAGNOSTICS_EVENT_COUNT,
    };

    const incremented = incrementDiagnosticsEventCount(nearLimit, "session_refresh_failed");
    const saturated = incrementDiagnosticsEventCount(incremented, "echo_failed");

    expect(incremented.session_refresh_failed).toBe(1);
    expect(saturated.echo_failed).toBe(MAX_DIAGNOSTICS_EVENT_COUNT);
  });
});
