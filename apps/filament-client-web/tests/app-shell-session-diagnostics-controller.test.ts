import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import { createSessionDiagnosticsController } from "../src/features/app-shell/runtime/session-diagnostics-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

function submitEventFixture(): SubmitEvent {
  return {
    preventDefault: vi.fn(),
  } as unknown as SubmitEvent;
}

describe("app shell session diagnostics controller", () => {
  it("refreshes sessions and maps status state", async () => {
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

    const setAuthenticatedSession = vi.fn();
    const clearAuthenticatedSession = vi.fn();
    const refreshAuthSessionMock = vi.fn(async () =>
      authSessionFromResponse({
        access_token: "C".repeat(64),
        refresh_token: "D".repeat(64),
        expires_in_secs: 3600,
      }),
    );

    const controller = createSessionDiagnosticsController(
      {
        session,
        setAuthenticatedSession,
        clearAuthenticatedSession,
        leaveVoiceChannel: vi.fn(async () => undefined),
        releaseRtcClient: vi.fn(async () => undefined),
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
        refreshAuthSession: refreshAuthSessionMock,
      },
    );

    await controller.refreshSession();

    expect(refreshAuthSessionMock).toHaveBeenCalledWith(SESSION.refreshToken);
    expect(setAuthenticatedSession).toHaveBeenCalledTimes(1);
    expect(sessionStatus()).toBe("Session refreshed.");
    expect(sessionError()).toBe("");
    expect(isRefreshingSession()).toBe(false);
    expect(clearAuthenticatedSession).not.toHaveBeenCalled();
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("session_refresh_succeeded");
  });

  it("clears local auth/cache on logout even when remote logout fails", async () => {
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
    const clearAuthenticatedSession = vi.fn();
    const clearWorkspaceCache = vi.fn();
    const logoutAuthSessionMock = vi.fn(async () => {
      throw new Error("offline");
    });

    const controller = createSessionDiagnosticsController(
      {
        session,
        setAuthenticatedSession: vi.fn(),
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
        logoutAuthSession: logoutAuthSessionMock,
        clearWorkspaceCache,
      },
    );

    await controller.logout();

    expect(leaveVoiceChannel).toHaveBeenCalledTimes(1);
    expect(releaseRtcClient).toHaveBeenCalledTimes(1);
    expect(logoutAuthSessionMock).toHaveBeenCalledWith(SESSION.refreshToken);
    expect(clearAuthenticatedSession).toHaveBeenCalledTimes(1);
    expect(clearWorkspaceCache).toHaveBeenCalledTimes(1);
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("logout_requested");
  });

  it("runs health and echo diagnostics with bounded state transitions", async () => {
    const [session] = createSignal(SESSION);
    const [isRefreshingSession, setRefreshingSession] = createSignal(false);
    const [sessionStatus, setSessionStatus] = createSignal("");
    const [sessionError, setSessionError] = createSignal("");
    const [isCheckingHealth, setCheckingHealth] = createSignal(false);
    const [healthStatus, setHealthStatus] = createSignal("");
    const [diagError, setDiagError] = createSignal("");
    const [isEchoing, setEchoing] = createSignal(false);
    const [echoInput] = createSignal("incident diagnostics payload");
    const recordDiagnosticsEvent = vi.fn();

    const fetchHealthMock = vi.fn(async () => ({ status: "ok" as const }));
    const echoMessageMock = vi.fn(async () => "incident diagnostics payload");

    const controller = createSessionDiagnosticsController(
      {
        session,
        setAuthenticatedSession: vi.fn(),
        clearAuthenticatedSession: vi.fn(),
        leaveVoiceChannel: vi.fn(async () => undefined),
        releaseRtcClient: vi.fn(async () => undefined),
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
        fetchHealth: fetchHealthMock,
        echoMessage: echoMessageMock,
      },
    );

    await controller.runHealthCheck();
    await controller.runEcho(submitEventFixture());

    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
    expect(echoMessageMock).toHaveBeenCalledWith({
      message: "incident diagnostics payload",
    });
    expect(healthStatus()).toBe("Echo: incident diagnostics payload");
    expect(diagError()).toBe("");
    expect(isCheckingHealth()).toBe(false);
    expect(isEchoing()).toBe(false);
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("health_check_succeeded");
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("echo_succeeded");
  });

  it("records failed diagnostics events without exposing payload details", async () => {
    const [session] = createSignal(SESSION);
    const [isRefreshingSession, setRefreshingSession] = createSignal(false);
    const [sessionStatus, setSessionStatus] = createSignal("");
    const [sessionError, setSessionError] = createSignal("");
    const [isCheckingHealth, setCheckingHealth] = createSignal(false);
    const [healthStatus, setHealthStatus] = createSignal("");
    const [diagError, setDiagError] = createSignal("");
    const [isEchoing, setEchoing] = createSignal(false);
    const [echoInput] = createSignal("payload that should not be logged");
    const recordDiagnosticsEvent = vi.fn();

    const refreshAuthSessionMock = vi.fn(async () => {
      throw new Error("refresh failed");
    });
    const fetchHealthMock = vi.fn(async () => {
      throw new Error("health failed");
    });
    const echoMessageMock = vi.fn(async () => {
      throw new Error("echo failed");
    });

    const controller = createSessionDiagnosticsController(
      {
        session,
        setAuthenticatedSession: vi.fn(),
        clearAuthenticatedSession: vi.fn(),
        leaveVoiceChannel: vi.fn(async () => undefined),
        releaseRtcClient: vi.fn(async () => undefined),
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
        refreshAuthSession: refreshAuthSessionMock,
        fetchHealth: fetchHealthMock,
        echoMessage: echoMessageMock,
      },
    );

    await controller.refreshSession();
    await controller.runHealthCheck();
    await controller.runEcho(submitEventFixture());

    expect(sessionStatus()).toBe("");
    expect(sessionError()).toBe("Unable to refresh session.");
    expect(diagError()).toBe("Echo request failed.");
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("session_refresh_failed");
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("health_check_failed");
    expect(recordDiagnosticsEvent).toHaveBeenCalledWith("echo_failed");
  });
});
