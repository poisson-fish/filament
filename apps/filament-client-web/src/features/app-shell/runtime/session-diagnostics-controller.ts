import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  echoMessage,
  fetchHealth,
  logoutAuthSession,
  refreshAuthSession,
} from "../../../lib/api";
import { clearWorkspaceCache } from "../../../lib/workspace-cache";
import { mapError } from "../helpers";
import type { DiagnosticsEventType } from "../state/diagnostics-event-counters";

export interface SessionDiagnosticsControllerOptions {
  session: Accessor<AuthSession | null>;
  setAuthenticatedSession: (session: AuthSession) => void;
  clearAuthenticatedSession: () => void;
  leaveVoiceChannel: () => Promise<void>;
  releaseRtcClient: () => Promise<void>;
  isRefreshingSession: Accessor<boolean>;
  setRefreshingSession: Setter<boolean>;
  setSessionStatus: Setter<string>;
  setSessionError: Setter<string>;
  isCheckingHealth: Accessor<boolean>;
  setCheckingHealth: Setter<boolean>;
  setHealthStatus: Setter<string>;
  setDiagError: Setter<string>;
  isEchoing: Accessor<boolean>;
  setEchoing: Setter<boolean>;
  echoInput: Accessor<string>;
  recordDiagnosticsEvent: (eventType: DiagnosticsEventType) => void;
}

export interface SessionDiagnosticsControllerDependencies {
  refreshAuthSession: typeof refreshAuthSession;
  logoutAuthSession: typeof logoutAuthSession;
  fetchHealth: typeof fetchHealth;
  echoMessage: typeof echoMessage;
  mapError: (error: unknown, fallback: string) => string;
  clearWorkspaceCache: () => void;
}

export interface SessionDiagnosticsController {
  refreshSession: () => Promise<void>;
  logout: () => Promise<void>;
  runHealthCheck: () => Promise<void>;
  runEcho: (event: SubmitEvent) => Promise<void>;
}

const DEFAULT_SESSION_DIAGNOSTICS_CONTROLLER_DEPENDENCIES: SessionDiagnosticsControllerDependencies =
  {
    refreshAuthSession,
    logoutAuthSession,
    fetchHealth,
    echoMessage,
    mapError,
    clearWorkspaceCache,
  };

export function createSessionDiagnosticsController(
  options: SessionDiagnosticsControllerOptions,
  dependencies: Partial<SessionDiagnosticsControllerDependencies> = {},
): SessionDiagnosticsController {
  const deps = {
    ...DEFAULT_SESSION_DIAGNOSTICS_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const refreshSession = async (): Promise<void> => {
    const session = options.session();
    if (!session || options.isRefreshingSession()) {
      return;
    }

    options.setRefreshingSession(true);
    options.setSessionError("");
    options.setSessionStatus("");
    try {
      const next = await deps.refreshAuthSession(session.refreshToken);
      options.setAuthenticatedSession(next);
      options.setSessionStatus("Session refreshed.");
      options.recordDiagnosticsEvent("session_refresh_succeeded");
    } catch (error) {
      options.setSessionError(deps.mapError(error, "Unable to refresh session."));
      options.recordDiagnosticsEvent("session_refresh_failed");
    } finally {
      options.setRefreshingSession(false);
    }
  };

  const logout = async (): Promise<void> => {
    options.recordDiagnosticsEvent("logout_requested");
    await options.leaveVoiceChannel();
    await options.releaseRtcClient();
    const session = options.session();
    if (session) {
      try {
        await deps.logoutAuthSession(session.refreshToken);
      } catch {
        // Best-effort remote session teardown; local session is still cleared.
      }
    }
    options.clearAuthenticatedSession();
    deps.clearWorkspaceCache();
  };

  const runHealthCheck = async (): Promise<void> => {
    if (options.isCheckingHealth()) {
      return;
    }

    options.setCheckingHealth(true);
    options.setDiagError("");
    try {
      const health = await deps.fetchHealth();
      options.setHealthStatus(`Health: ${health.status}`);
      options.recordDiagnosticsEvent("health_check_succeeded");
    } catch (error) {
      options.setDiagError(deps.mapError(error, "Health check failed."));
      options.recordDiagnosticsEvent("health_check_failed");
    } finally {
      options.setCheckingHealth(false);
    }
  };

  const runEcho = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    if (options.isEchoing()) {
      return;
    }

    options.setEchoing(true);
    options.setDiagError("");
    try {
      const echoed = await deps.echoMessage({ message: options.echoInput() });
      options.setHealthStatus(`Echo: ${echoed.slice(0, 60)}`);
      options.recordDiagnosticsEvent("echo_succeeded");
    } catch (error) {
      options.setDiagError(deps.mapError(error, "Echo request failed."));
      options.recordDiagnosticsEvent("echo_failed");
    } finally {
      options.setEchoing(false);
    }
  };

  return {
    refreshSession,
    logout,
    runHealthCheck,
    runEcho,
  };
}
