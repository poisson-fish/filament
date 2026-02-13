import { describe, expect, it, vi } from "vitest";
import { createUtilityPanelProps } from "../src/features/app-shell/runtime/utility-panel-props";

describe("app shell utility panel props", () => {
  it("maps utility values and handlers", async () => {
    const setEchoInput = vi.fn();
    const onRunHealthCheck = vi.fn();
    const onRunEcho = vi.fn();

    const panelProps = createUtilityPanelProps({
      echoInput: "ping",
      healthStatus: "ok",
      diagError: "",
      isCheckingHealth: false,
      isEchoing: false,
      setEchoInput,
      onRunHealthCheck,
      onRunEcho,
    });

    expect(panelProps.echoInput).toBe("ping");
    expect(panelProps.healthStatus).toBe("ok");
    expect(panelProps.diagError).toBe("");
    expect(panelProps.isCheckingHealth).toBe(false);
    expect(panelProps.isEchoing).toBe(false);

    panelProps.setEchoInput("pong");
    expect(setEchoInput).toHaveBeenCalledWith("pong");

    await panelProps.onRunHealthCheck();
    expect(onRunHealthCheck).toHaveBeenCalledTimes(1);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onRunEcho(submitEvent);
    expect(onRunEcho).toHaveBeenCalledWith(submitEvent);
  });
});