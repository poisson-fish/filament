import { createSignal } from "solid-js";
import type { RoleName } from "../../../domain/chat";

export function createDiagnosticsState() {
  const [moderationUserIdInput, setModerationUserIdInput] = createSignal("");
  const [moderationRoleInput, setModerationRoleInput] = createSignal<RoleName>("member");
  const [isModerating, setModerating] = createSignal(false);
  const [moderationStatus, setModerationStatus] = createSignal("");
  const [moderationError, setModerationError] = createSignal("");

  const [overrideRoleInput, setOverrideRoleInput] = createSignal<RoleName>("member");
  const [overrideAllowCsv, setOverrideAllowCsv] = createSignal("create_message");
  const [overrideDenyCsv, setOverrideDenyCsv] = createSignal("");

  const [isRefreshingSession, setRefreshingSession] = createSignal(false);
  const [sessionStatus, setSessionStatus] = createSignal("");
  const [sessionError, setSessionError] = createSignal("");

  const [healthStatus, setHealthStatus] = createSignal("");
  const [echoInput, setEchoInput] = createSignal("hello filament");
  const [diagError, setDiagError] = createSignal("");
  const [isCheckingHealth, setCheckingHealth] = createSignal(false);
  const [isEchoing, setEchoing] = createSignal(false);

  return {
    moderationUserIdInput,
    setModerationUserIdInput,
    moderationRoleInput,
    setModerationRoleInput,
    isModerating,
    setModerating,
    moderationStatus,
    setModerationStatus,
    moderationError,
    setModerationError,
    overrideRoleInput,
    setOverrideRoleInput,
    overrideAllowCsv,
    setOverrideAllowCsv,
    overrideDenyCsv,
    setOverrideDenyCsv,
    isRefreshingSession,
    setRefreshingSession,
    sessionStatus,
    setSessionStatus,
    sessionError,
    setSessionError,
    healthStatus,
    setHealthStatus,
    echoInput,
    setEchoInput,
    diagError,
    setDiagError,
    isCheckingHealth,
    setCheckingHealth,
    isEchoing,
    setEchoing,
  };
}
