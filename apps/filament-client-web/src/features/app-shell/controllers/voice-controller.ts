import type { RtcSnapshot } from "../../../lib/rtc";

export type AudioDeviceKind = "audioinput" | "audiooutput";

export interface VoiceConnectionTransitionInput {
  previousStatus: RtcSnapshot["connectionStatus"];
  currentStatus: RtcSnapshot["connectionStatus"];
  hasConnectedChannel: boolean;
  isJoining: boolean;
  isLeaving: boolean;
}

export interface VoiceConnectionTransitionResult {
  shouldClearSession: boolean;
  statusMessage: string;
  errorMessage: string;
}

export function resolveVoiceConnectionTransition(
  input: VoiceConnectionTransitionInput,
): VoiceConnectionTransitionResult {
  if (input.hasConnectedChannel && input.currentStatus === "reconnecting") {
    return {
      shouldClearSession: false,
      statusMessage: "Voice reconnecting. Media may recover automatically.",
      errorMessage: "",
    };
  }

  if (
    input.hasConnectedChannel &&
    input.currentStatus === "connected" &&
    input.previousStatus === "reconnecting"
  ) {
    return {
      shouldClearSession: false,
      statusMessage: "Voice reconnected.",
      errorMessage: "",
    };
  }

  if (
    input.hasConnectedChannel &&
    input.currentStatus === "disconnected" &&
    input.previousStatus !== "disconnected" &&
    !input.isJoining &&
    !input.isLeaving
  ) {
    return {
      shouldClearSession: true,
      statusMessage: "",
      errorMessage: "Voice connection dropped. Select Join Voice to reconnect.",
    };
  }

  return {
    shouldClearSession: false,
    statusMessage: "",
    errorMessage: "",
  };
}

export function unavailableVoiceDeviceError(kind: AudioDeviceKind): string {
  if (kind === "audioinput") {
    return "Selected microphone is not available.";
  }
  return "Selected speaker is not available.";
}

export function resolveVoiceDevicePreferenceStatus(
  kind: AudioDeviceKind,
  isVoiceSessionActive: boolean,
  nextDeviceId: string | null,
): string {
  if (!isVoiceSessionActive) {
    if (kind === "audioinput") {
      return "Microphone preference saved for the next voice join.";
    }
    return "Speaker preference saved for the next voice join.";
  }

  if (nextDeviceId) {
    if (kind === "audioinput") {
      return "Microphone updated for the active voice session.";
    }
    return "Speaker updated for the active voice session.";
  }

  if (kind === "audioinput") {
    return "Microphone preference cleared. Current session keeps its current device.";
  }
  return "Speaker preference cleared. Current session keeps its current device.";
}
