import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { UtilityPanel } from "../src/features/app-shell/components/panels/UtilityPanel";

function baseProps(overrides: Partial<Parameters<typeof UtilityPanel>[0]> = {}) {
  return {
    echoInput: "ping",
    healthStatus: "Health: ok",
    diagError: "",
    diagnosticsEventCounts: {
      session_refresh_succeeded: 2,
      session_refresh_failed: 1,
      health_check_succeeded: 3,
      health_check_failed: 0,
      echo_succeeded: 4,
      echo_failed: 1,
      logout_requested: 2,
      gateway_connected: 5,
      gateway_disconnected: 3,
    },
    showDiagnosticsCounters: false,
    isCheckingHealth: false,
    isEchoing: false,
    onEchoInput: vi.fn(),
    onRunHealthCheck: vi.fn(),
    onRunEcho: vi.fn((event: SubmitEvent) => event.preventDefault()),
    ...overrides,
  };
}

describe("app shell utility panel", () => {
  it("hides diagnostics counters when dev mode is disabled", () => {
    render(() => <UtilityPanel {...baseProps({ showDiagnosticsCounters: false })} />);

    expect(screen.queryByText("Diagnostics counters (dev only)")).toBeNull();
  });

  it("shows diagnostics counters in dev mode and keeps actions wired", () => {
    const onRunHealthCheck = vi.fn();
    const onRunEcho = vi.fn((event: SubmitEvent) => event.preventDefault());

    render(() =>
      <UtilityPanel
        {...baseProps({
          showDiagnosticsCounters: true,
          onRunHealthCheck,
          onRunEcho,
        })}
      />,
    );

    expect(screen.getByText("Diagnostics counters (dev only)")).toBeInTheDocument();
    expect(screen.getByText("Session refresh: 2 ok / 1 failed")).toBeInTheDocument();
    expect(screen.getByText("Logout requests: 2")).toBeInTheDocument();
    expect(screen.getByText("Gateway connections: 5 opened / 3 closed")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Health" }));
    expect(onRunHealthCheck).toHaveBeenCalledTimes(1);

    const form = screen.getByRole("button", { name: "Echo" }).closest("form");
    expect(form).not.toBeNull();
    fireEvent.submit(form!);
    expect(onRunEcho).toHaveBeenCalledTimes(1);
  });
});
