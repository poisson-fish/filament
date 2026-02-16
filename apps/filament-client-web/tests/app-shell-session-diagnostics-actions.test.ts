import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import { createSessionDiagnosticsActions } from "../src/features/app-shell/runtime/session-diagnostics-actions";
import type { SessionDiagnosticsControllerOptions } from "../src/features/app-shell/runtime/session-diagnostics-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

describe("app shell session diagnostics actions", () => {
  it("wires runtime diagnostics state into session diagnostics controller", async () => {
    const [session] = createSignal(SESSION);
    const [isRefreshingSession, setRefreshingSession] = createSignal(false);
    const [sessionStatus, setSessionStatus] = createSignal("");
    const [sessionError, setSessionError] = createSignal("");
    const [isCheckingHealth, setCheckingHealth] = createSignal(false);
    const [healthStatus, setHealthStatus] = createSignal("");
    const [diagError, setDiagError] = createSignal("");
    const [isEchoing, setEchoing] = createSignal(false);
    const [echoInput] = createSignal("ping");
    const recordDiagnosticsEvent = vi.fn();

    const leaveVoiceChannel = vi.fn(async () => undefined);
    const releaseRtcClient = vi.fn(async () => undefined);
    const setAuthenticatedSession = vi.fn();
    const clearAuthenticatedSession = vi.fn();

    const controller = {
      refreshSession: vi.fn(async () => undefined),
      logout: vi.fn(async () => undefined),
      runHealthCheck: vi.fn(async () => undefined),
      runEcho: vi.fn(async () => undefined),
    };

    const createSessionDiagnosticsControllerMock = vi.fn(
      (options: SessionDiagnosticsControllerOptions) => {
        void options.leaveVoiceChannel();
        void options.releaseRtcClient();
        options.setSessionStatus("wired");
        return controller;
      },
    );

    const result = createSessionDiagnosticsActions(
      {
        session,
        setAuthenticatedSession,
        clearAuthenticatedSession,
        leaveVoiceChannel,
        releaseRtcClient,
        isRefreshingSession,
        setRefreshingSession,
        setSessionStatus,
        setSessionError,
        isCheckingHealth,
        setCheckingHealth,
        setHealthStatus,
        setDiagError,
        isEchoing,
        setEchoing,
        echoInput,
        recordDiagnosticsEvent,
      },
      {
        createSessionDiagnosticsController: createSessionDiagnosticsControllerMock,
      },
    );

    expect(result).toBe(controller);
    expect(createSessionDiagnosticsControllerMock).toHaveBeenCalledTimes(1);
    expect(leaveVoiceChannel).toHaveBeenCalledTimes(1);
    expect(releaseRtcClient).toHaveBeenCalledTimes(1);
    expect(sessionStatus()).toBe("wired");
    expect(createSessionDiagnosticsControllerMock).toHaveBeenCalledWith(
      expect.objectContaining({
        recordDiagnosticsEvent,
      }),
    );
  });
});
