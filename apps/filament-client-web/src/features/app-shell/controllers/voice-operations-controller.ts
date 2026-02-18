import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  ChannelRecord,
  GuildId,
  MediaPublishSource,
  VoiceTokenRecord,
} from "../../../domain/chat";
import {
  issueVoiceToken,
  leaveVoiceChannel as leaveVoiceChannelApi,
} from "../../../lib/api";
import { createRtcClient, type RtcClient, type RtcSnapshot } from "../../../lib/rtc";
import type { VoiceDevicePreferences } from "../../../lib/voice-device-settings";
import {
  channelKey,
  mapRtcError,
  mapVoiceJoinError,
} from "../helpers";
import {
  reduceAsyncOperationState,
  type AsyncOperationEvent,
  type AsyncOperationState,
} from "../state/async-operation-state";
import type { VoiceSessionCapabilities } from "../types";
import { RTC_DISCONNECTED_SNAPSHOT } from "../config/ui-constants";

export interface VoiceOperationsControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannel: Accessor<ChannelRecord | null>;
  voiceSessionChannelKey: Accessor<string | null>;
  canPublishVoiceCamera: Accessor<boolean>;
  canPublishVoiceScreenShare: Accessor<boolean>;
  canSubscribeVoiceStreams: Accessor<boolean>;
  canToggleVoiceCamera: Accessor<boolean>;
  canToggleVoiceScreenShare: Accessor<boolean>;
  isJoiningVoice: Accessor<boolean>;
  isLeavingVoice: Accessor<boolean>;
  isTogglingVoiceMic: Accessor<boolean>;
  isTogglingVoiceDeaf: Accessor<boolean>;
  isTogglingVoiceCamera: Accessor<boolean>;
  isTogglingVoiceScreenShare: Accessor<boolean>;
  voiceDevicePreferences: Accessor<VoiceDevicePreferences>;
  setRtcSnapshot: Setter<RtcSnapshot>;
  setVoiceStatus: Setter<string>;
  setVoiceError: Setter<string>;
  setVoiceJoinState: Setter<AsyncOperationState>;
  setLeavingVoice: Setter<boolean>;
  setTogglingVoiceMic: Setter<boolean>;
  setTogglingVoiceDeaf: Setter<boolean>;
  setTogglingVoiceCamera: Setter<boolean>;
  setTogglingVoiceScreenShare: Setter<boolean>;
  setVoiceSessionChannelKey: Setter<string | null>;
  setVoiceSessionStartedAtUnixMs: Setter<number | null>;
  setVoiceDurationClockUnixMs: Setter<number>;
  setVoiceSessionCapabilities: Setter<VoiceSessionCapabilities>;
  setAudioDevicesError: Setter<string>;
  defaultVoiceSessionCapabilities: VoiceSessionCapabilities;
}

export interface VoiceOperationsControllerDependencies {
  issueVoiceToken: (
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelRecord["channelId"],
    input: {
      canSubscribe?: boolean;
      publishSources?: MediaPublishSource[];
    },
  ) => Promise<VoiceTokenRecord>;
  leaveVoiceChannel: (
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelRecord["channelId"],
  ) => Promise<void>;
  createRtcClient: () => RtcClient;
  channelKey: (guildId: GuildId, channelId: ChannelRecord["channelId"]) => string;
  mapRtcError: (error: unknown, fallback: string) => string;
  mapVoiceJoinError: (error: unknown) => string;
  now: () => number;
}

export interface VoiceOperationsController {
  ensureRtcClient: () => RtcClient;
  releaseRtcClient: () => Promise<void>;
  peekRtcClient: () => RtcClient | null;
  joinVoiceChannel: () => Promise<void>;
  leaveVoiceChannel: (statusMessage?: string) => Promise<void>;
  toggleVoiceMicrophone: () => Promise<void>;
  toggleVoiceDeafen: () => Promise<void>;
  toggleVoiceCamera: () => Promise<void>;
  toggleVoiceScreenShare: () => Promise<void>;
}

const DEFAULT_VOICE_OPERATIONS_CONTROLLER_DEPENDENCIES: VoiceOperationsControllerDependencies = {
  issueVoiceToken,
  leaveVoiceChannel: leaveVoiceChannelApi,
  createRtcClient,
  channelKey,
  mapRtcError,
  mapVoiceJoinError,
  now: () => Date.now(),
};

export function createVoiceOperationsController(
  options: VoiceOperationsControllerOptions,
  dependencies: Partial<VoiceOperationsControllerDependencies> = {},
): VoiceOperationsController {
  const deps = {
    ...DEFAULT_VOICE_OPERATIONS_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  let rtcClient: RtcClient | null = null;
  let stopRtcSubscription: (() => void) | null = null;

  const applyVoiceJoinTransition = (event: AsyncOperationEvent) => {
    options.setVoiceJoinState((existing) => {
      const next = reduceAsyncOperationState(existing, event);
      options.setVoiceStatus(next.statusMessage);
      options.setVoiceError(next.errorMessage);
      return next;
    });
  };

  const resetVoiceSessionState = () => {
    options.setVoiceSessionChannelKey(null);
    options.setVoiceSessionStartedAtUnixMs(null);
    options.setRtcSnapshot(RTC_DISCONNECTED_SNAPSHOT);
    options.setVoiceSessionCapabilities(options.defaultVoiceSessionCapabilities);
  };

  const ensureRtcClient = (): RtcClient => {
    if (rtcClient) {
      return rtcClient;
    }
    rtcClient = deps.createRtcClient();
    stopRtcSubscription = rtcClient.subscribe((snapshot) => {
      options.setRtcSnapshot(snapshot);
    });
    return rtcClient;
  };

  const releaseRtcClient = async (): Promise<void> => {
    if (stopRtcSubscription) {
      stopRtcSubscription();
      stopRtcSubscription = null;
    }
    if (rtcClient) {
      try {
        await rtcClient.destroy();
      } catch {
        // Local state is reset regardless of transport teardown failures.
      } finally {
        rtcClient = null;
      }
    }
    resetVoiceSessionState();
  };

  const leaveVoiceChannel = async (statusMessage?: string): Promise<void> => {
    if (options.isLeavingVoice()) {
      return;
    }
    options.setLeavingVoice(true);
    const connectedChannelKey = options.voiceSessionChannelKey();
    const session = options.session();
    try {
      if (rtcClient) {
        await rtcClient.leave();
      }
      if (session && connectedChannelKey) {
        const [guildId, channelId] = connectedChannelKey.split("|");
        if (guildId && channelId) {
          await deps.leaveVoiceChannel(
            session,
            guildId as GuildId,
            channelId as ChannelRecord["channelId"],
          );
        }
      }
    } catch {
      // Local session teardown is deterministic even if leave fails.
    } finally {
      resetVoiceSessionState();
      applyVoiceJoinTransition({
        type: "reset",
      });
      if (statusMessage) {
        options.setVoiceStatus(statusMessage);
      }
      options.setLeavingVoice(false);
    }
  };

  const joinVoiceChannel = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channel = options.activeChannel();
    if (options.isJoiningVoice() || options.isLeavingVoice()) {
      return;
    }

    if (
      !session ||
      !guildId ||
      !channel ||
      channel.kind !== "voice"
    ) {
      applyVoiceJoinTransition({
        type: "reset",
      });
      return;
    }

    const targetVoiceSessionChannelKey = deps.channelKey(guildId, channel.channelId);
    if (options.voiceSessionChannelKey() === targetVoiceSessionChannelKey) {
      applyVoiceJoinTransition({
        type: "succeed",
        statusMessage: "Voice connected.",
      });
      return;
    }

    applyVoiceJoinTransition({
      type: "start",
    });
    options.setVoiceSessionCapabilities(options.defaultVoiceSessionCapabilities);
    try {
      const requestedPublishSources: MediaPublishSource[] = ["microphone"];
      if (options.canPublishVoiceCamera()) {
        requestedPublishSources.push("camera");
      }
      if (options.canPublishVoiceScreenShare()) {
        requestedPublishSources.push("screen_share");
      }
      const token = await deps.issueVoiceToken(session, guildId, channel.channelId, {
        canSubscribe: options.canSubscribeVoiceStreams(),
        publishSources: requestedPublishSources,
      });

      const client = ensureRtcClient();
      const preferences = options.voiceDevicePreferences();
      await client.setAudioInputDevice(preferences.audioInputDeviceId);
      await client.setAudioOutputDevice(preferences.audioOutputDeviceId);
      await client.join({
        livekitUrl: token.livekitUrl,
        token: token.token,
      });

      const startedAt = deps.now();
      options.setVoiceSessionChannelKey(targetVoiceSessionChannelKey);
      options.setVoiceSessionStartedAtUnixMs(startedAt);
      options.setVoiceDurationClockUnixMs(startedAt);
      options.setVoiceSessionCapabilities({
        canSubscribe: token.canSubscribe,
        publishSources: [...token.publishSources],
      });

      const joinSnapshot = client.snapshot();
      if (
        joinSnapshot.lastErrorCode === "audio_device_switch_failed" &&
        joinSnapshot.lastErrorMessage
      ) {
        options.setAudioDevicesError(joinSnapshot.lastErrorMessage);
      }

      if (token.canPublish && token.publishSources.includes("microphone")) {
        try {
          await client.setMicrophoneEnabled(true);
          applyVoiceJoinTransition({
            type: "succeed",
            statusMessage: "Voice connected. Microphone enabled.",
          });
        } catch (error) {
          applyVoiceJoinTransition({
            type: "succeed",
            statusMessage: "Voice connected.",
          });
          options.setVoiceError(
            deps.mapRtcError(error, "Connected, but microphone activation failed."),
          );
        }
        return;
      }

      applyVoiceJoinTransition({
        type: "succeed",
        statusMessage: "Voice connected in listen-only mode.",
      });
    } catch (error) {
      const errorMessage = deps.mapVoiceJoinError(error);
      applyVoiceJoinTransition({
        type: "fail",
        errorMessage,
      });
    }
  };

  const toggleVoiceMicrophone = async (): Promise<void> => {
    if (!rtcClient || options.isTogglingVoiceMic()) {
      return;
    }
    options.setTogglingVoiceMic(true);
    options.setVoiceError("");
    try {
      const enabled = await rtcClient.toggleMicrophone();
      options.setVoiceStatus(enabled ? "Microphone unmuted." : "Microphone muted.");
    } catch (error) {
      options.setVoiceError(deps.mapRtcError(error, "Unable to update microphone."));
    } finally {
      options.setTogglingVoiceMic(false);
    }
  };

  const toggleVoiceDeafen = async (): Promise<void> => {
    if (!rtcClient || options.isTogglingVoiceDeaf()) {
      return;
    }
    options.setTogglingVoiceDeaf(true);
    options.setVoiceError("");
    try {
      const enabled = await rtcClient.toggleDeafened();
      options.setVoiceStatus(enabled ? "Audio output deafened." : "Audio output undeafened.");
    } catch (error) {
      options.setVoiceError(deps.mapRtcError(error, "Unable to update deafen state."));
    } finally {
      options.setTogglingVoiceDeaf(false);
    }
  };

  const toggleVoiceCamera = async (): Promise<void> => {
    if (!rtcClient || options.isTogglingVoiceCamera()) {
      return;
    }
    if (!options.canToggleVoiceCamera()) {
      options.setVoiceError("Camera publish is not allowed for this call.");
      return;
    }

    options.setTogglingVoiceCamera(true);
    options.setVoiceError("");
    try {
      const enabled = await rtcClient.toggleCamera();
      options.setVoiceStatus(enabled ? "Camera enabled." : "Camera disabled.");
    } catch (error) {
      options.setVoiceError(deps.mapRtcError(error, "Unable to update camera."));
    } finally {
      options.setTogglingVoiceCamera(false);
    }
  };

  const toggleVoiceScreenShare = async (): Promise<void> => {
    if (!rtcClient || options.isTogglingVoiceScreenShare()) {
      return;
    }
    if (!options.canToggleVoiceScreenShare()) {
      options.setVoiceError("Screen share publish is not allowed for this call.");
      return;
    }

    options.setTogglingVoiceScreenShare(true);
    options.setVoiceError("");
    try {
      const enabled = await rtcClient.toggleScreenShare();
      options.setVoiceStatus(enabled ? "Screen share enabled." : "Screen share stopped.");
    } catch (error) {
      options.setVoiceError(deps.mapRtcError(error, "Unable to update screen share."));
    } finally {
      options.setTogglingVoiceScreenShare(false);
    }
  };

  return {
    ensureRtcClient,
    releaseRtcClient,
    peekRtcClient: () => rtcClient,
    joinVoiceChannel,
    leaveVoiceChannel,
    toggleVoiceMicrophone,
    toggleVoiceDeafen,
    toggleVoiceCamera,
    toggleVoiceScreenShare,
  };
}
