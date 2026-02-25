import { createEffect, type Accessor } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type { VoiceSessionCapabilities } from "../types";

export interface VoicePermissionState {
  canSubscribe: boolean;
  canPublishCamera: boolean;
  canPublishScreenShare: boolean;
}

export interface VoicePermissionSyncControllerOptions {
  session: Accessor<AuthSession | null>;
  isVoiceSessionActive: Accessor<boolean>;
  isVoiceSessionForActiveChannel: Accessor<boolean>;
  voiceSessionChannelKey: Accessor<string | null>;
  isJoiningVoice: Accessor<boolean>;
  isLeavingVoice: Accessor<boolean>;
  canPublishVoiceCamera: Accessor<boolean>;
  canPublishVoiceScreenShare: Accessor<boolean>;
  canSubscribeVoiceStreams: Accessor<boolean>;
  voiceSessionCapabilities: Accessor<VoiceSessionCapabilities>;
  refreshVoiceSessionPermissions: () => Promise<void>;
}

export function resolveDesiredVoicePermissionState(
  input: Pick<
    VoicePermissionSyncControllerOptions,
    | "canPublishVoiceCamera"
    | "canPublishVoiceScreenShare"
    | "canSubscribeVoiceStreams"
  >,
): VoicePermissionState {
  return {
    canSubscribe: input.canSubscribeVoiceStreams(),
    canPublishCamera: input.canPublishVoiceCamera(),
    canPublishScreenShare: input.canPublishVoiceScreenShare(),
  };
}

export function hasVoiceSessionPermissionMismatch(
  desired: VoicePermissionState,
  capabilities: VoiceSessionCapabilities,
): boolean {
  const canPublishCamera = capabilities.publishSources.includes("camera");
  const canPublishScreenShare = capabilities.publishSources.includes("screen_share");
  return (
    desired.canSubscribe !== capabilities.canSubscribe ||
    desired.canPublishCamera !== canPublishCamera ||
    desired.canPublishScreenShare !== canPublishScreenShare
  );
}

function voicePermissionStateSignature(
  channelKey: string,
  state: VoicePermissionState,
): string {
  return [
    channelKey,
    state.canSubscribe ? "sub:1" : "sub:0",
    state.canPublishCamera ? "cam:1" : "cam:0",
    state.canPublishScreenShare ? "screen:1" : "screen:0",
  ].join("|");
}

export function createVoicePermissionSyncController(
  options: VoicePermissionSyncControllerOptions,
): void {
  let refreshing = false;
  let lastAttemptedSignature: string | null = null;

  createEffect(() => {
    const session = options.session();
    const channelKey = options.voiceSessionChannelKey();
    const desired = resolveDesiredVoicePermissionState(options);
    const capabilities = options.voiceSessionCapabilities();
    const shouldIgnore =
      !session ||
      !options.isVoiceSessionActive() ||
      !options.isVoiceSessionForActiveChannel() ||
      !channelKey ||
      options.isJoiningVoice() ||
      options.isLeavingVoice();
    if (shouldIgnore) {
      if (!refreshing) {
        lastAttemptedSignature = null;
      }
      return;
    }

    const mismatch = hasVoiceSessionPermissionMismatch(desired, capabilities);
    if (!mismatch) {
      lastAttemptedSignature = null;
      return;
    }

    if (refreshing) {
      return;
    }

    const signature = voicePermissionStateSignature(channelKey, desired);
    if (lastAttemptedSignature === signature) {
      return;
    }

    refreshing = true;
    lastAttemptedSignature = signature;
    void options.refreshVoiceSessionPermissions().finally(() => {
      refreshing = false;
    });
  });
}
