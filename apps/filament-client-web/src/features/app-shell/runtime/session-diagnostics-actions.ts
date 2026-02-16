import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  createSessionDiagnosticsController,
  type SessionDiagnosticsController,
} from "./session-diagnostics-controller";
import type { DiagnosticsEventType } from "../state/diagnostics-event-counters";

export interface SessionDiagnosticsActionsOptions {
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

export interface SessionDiagnosticsActionsDependencies {
  createSessionDiagnosticsController: typeof createSessionDiagnosticsController;
}

const DEFAULT_SESSION_DIAGNOSTICS_ACTIONS_DEPENDENCIES: SessionDiagnosticsActionsDependencies =
  {
    createSessionDiagnosticsController,
  };

export function createSessionDiagnosticsActions(
  options: SessionDiagnosticsActionsOptions,
  dependencies: Partial<SessionDiagnosticsActionsDependencies> = {},
): SessionDiagnosticsController {
  const deps = {
    ...DEFAULT_SESSION_DIAGNOSTICS_ACTIONS_DEPENDENCIES,
    ...dependencies,
  };

  return deps.createSessionDiagnosticsController({
    session: options.session,
    setAuthenticatedSession: options.setAuthenticatedSession,
    clearAuthenticatedSession: options.clearAuthenticatedSession,
    leaveVoiceChannel: () => options.leaveVoiceChannel(),
    releaseRtcClient: () => options.releaseRtcClient(),
    isRefreshingSession: options.isRefreshingSession,
    setRefreshingSession: options.setRefreshingSession,
    setSessionStatus: options.setSessionStatus,
    setSessionError: options.setSessionError,
    isCheckingHealth: options.isCheckingHealth,
    setCheckingHealth: options.setCheckingHealth,
    setHealthStatus: options.setHealthStatus,
    setDiagError: options.setDiagError,
    isEchoing: options.isEchoing,
    setEchoing: options.setEchoing,
    echoInput: options.echoInput,
    recordDiagnosticsEvent: options.recordDiagnosticsEvent,
  });
}
