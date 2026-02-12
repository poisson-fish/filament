import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type { WorkspaceRecord } from "../../../domain/chat";
import type { RtcSnapshot } from "../../../lib/rtc";
import { parseChannelKey } from "../helpers";
import type { VoiceSessionCapabilities } from "../types";

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

export interface VoiceSessionLifecycleControllerOptions {
  session: Accessor<AuthSession | null>;
  workspaces: Accessor<WorkspaceRecord[]>;
  rtcSnapshot: Accessor<RtcSnapshot>;
  isVoiceSessionActive: Accessor<boolean>;
  voiceSessionChannelKey: Accessor<string | null>;
  voiceSessionStartedAtUnixMs: Accessor<number | null>;
  isJoiningVoice: Accessor<boolean>;
  isLeavingVoice: Accessor<boolean>;
  leaveVoiceChannel: () => Promise<void>;
  setVoiceDurationClockUnixMs: Setter<number>;
  setVoiceSessionChannelKey: Setter<string | null>;
  setVoiceSessionStartedAtUnixMs: Setter<number | null>;
  setVoiceSessionCapabilities: Setter<VoiceSessionCapabilities>;
  defaultVoiceSessionCapabilities: VoiceSessionCapabilities;
  setVoiceStatus: Setter<string>;
  setVoiceError: Setter<string>;
}

export function createVoiceSessionLifecycleController(
  options: VoiceSessionLifecycleControllerOptions,
): void {
  createEffect(() => {
    if (
      !options.isVoiceSessionActive() ||
      !options.voiceSessionStartedAtUnixMs()
    ) {
      return;
    }
    options.setVoiceDurationClockUnixMs(Date.now());
    const timer = window.setInterval(() => {
      options.setVoiceDurationClockUnixMs(Date.now());
    }, 1000);
    onCleanup(() => window.clearInterval(timer));
  });

  createEffect(() => {
    const session = options.session();
    const connectedChannelKey = options.voiceSessionChannelKey();
    if (!connectedChannelKey || options.isLeavingVoice()) {
      return;
    }
    if (!session) {
      void options.leaveVoiceChannel();
      return;
    }
    const connected = parseChannelKey(connectedChannelKey);
    if (!connected) {
      void options.leaveVoiceChannel();
      return;
    }
    const workspace =
      options.workspaces().find((entry) => entry.guildId === connected.guildId) ??
      null;
    const voiceChannelStillVisible = workspace?.channels.some(
      (channel) =>
        channel.channelId === connected.channelId && channel.kind === "voice",
    );
    if (!workspace || !voiceChannelStillVisible) {
      void options.leaveVoiceChannel();
    }
  });

  let previousVoiceConnectionStatus: RtcSnapshot["connectionStatus"] =
    options.rtcSnapshot().connectionStatus;

  createEffect(() => {
    const snapshot = options.rtcSnapshot();
    const transition = resolveVoiceConnectionTransition({
      previousStatus: previousVoiceConnectionStatus,
      currentStatus: snapshot.connectionStatus,
      hasConnectedChannel: Boolean(options.voiceSessionChannelKey()),
      isJoining: options.isJoiningVoice(),
      isLeaving: options.isLeavingVoice(),
    });

    if (transition.shouldClearSession) {
      options.setVoiceSessionChannelKey(null);
      options.setVoiceSessionStartedAtUnixMs(null);
      options.setVoiceSessionCapabilities(options.defaultVoiceSessionCapabilities);
    }
    if (transition.statusMessage) {
      options.setVoiceStatus(transition.statusMessage);
      options.setVoiceError("");
    }
    if (transition.errorMessage) {
      options.setVoiceStatus("");
      options.setVoiceError(transition.errorMessage);
    }

    previousVoiceConnectionStatus = snapshot.connectionStatus;
  });
}
