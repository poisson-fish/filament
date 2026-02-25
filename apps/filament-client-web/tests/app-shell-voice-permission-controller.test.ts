import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  createVoicePermissionSyncController,
  hasVoiceSessionPermissionMismatch,
  resolveDesiredVoicePermissionState,
} from "../src/features/app-shell/controllers/voice-permission-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell voice permission controller", () => {
  it("triggers a one-shot permission refresh when token capabilities drift from desired permissions", async () => {
    const refreshVoiceSessionPermissions = vi.fn(async () => undefined);
    const [session] = createSignal(SESSION);
    const [isVoiceSessionActive] = createSignal(true);
    const [isVoiceSessionForActiveChannel] = createSignal(true);
    const [voiceSessionChannelKey] = createSignal("guild-a|channel-a");
    const [isJoiningVoice] = createSignal(false);
    const [isLeavingVoice] = createSignal(false);
    const [canPublishVoiceCamera] = createSignal(false);
    const [canPublishVoiceScreenShare] = createSignal(true);
    const [canSubscribeVoiceStreams] = createSignal(true);
    const [voiceSessionCapabilities, setVoiceSessionCapabilities] = createSignal({
      canSubscribe: true,
      publishSources: ["microphone"] as ("microphone" | "camera" | "screen_share")[],
    });

    const dispose = createRoot((innerDispose) => {
      createVoicePermissionSyncController({
        session,
        isVoiceSessionActive,
        isVoiceSessionForActiveChannel,
        voiceSessionChannelKey,
        isJoiningVoice,
        isLeavingVoice,
        canPublishVoiceCamera,
        canPublishVoiceScreenShare,
        canSubscribeVoiceStreams,
        voiceSessionCapabilities,
        refreshVoiceSessionPermissions,
      });
      return innerDispose;
    });

    await flush();
    expect(refreshVoiceSessionPermissions).toHaveBeenCalledTimes(1);

    setVoiceSessionCapabilities({
      canSubscribe: true,
      publishSources: ["microphone"] as ("microphone" | "camera" | "screen_share")[],
    });
    await flush();
    expect(refreshVoiceSessionPermissions).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("does not refresh when desired permissions already match session capabilities", async () => {
    const refreshVoiceSessionPermissions = vi.fn(async () => undefined);
    const [session] = createSignal(SESSION);
    const [isVoiceSessionActive] = createSignal(true);
    const [isVoiceSessionForActiveChannel] = createSignal(true);
    const [voiceSessionChannelKey] = createSignal("guild-a|channel-a");
    const [isJoiningVoice] = createSignal(false);
    const [isLeavingVoice] = createSignal(false);
    const [canPublishVoiceCamera] = createSignal(true);
    const [canPublishVoiceScreenShare] = createSignal(true);
    const [canSubscribeVoiceStreams] = createSignal(true);
    const [voiceSessionCapabilities] = createSignal({
      canSubscribe: true,
      publishSources: ["microphone", "camera", "screen_share"] as (
        "microphone" | "camera" | "screen_share"
      )[],
    });

    const dispose = createRoot((innerDispose) => {
      createVoicePermissionSyncController({
        session,
        isVoiceSessionActive,
        isVoiceSessionForActiveChannel,
        voiceSessionChannelKey,
        isJoiningVoice,
        isLeavingVoice,
        canPublishVoiceCamera,
        canPublishVoiceScreenShare,
        canSubscribeVoiceStreams,
        voiceSessionCapabilities,
        refreshVoiceSessionPermissions,
      });
      return innerDispose;
    });

    await flush();
    expect(refreshVoiceSessionPermissions).not.toHaveBeenCalled();
    dispose();
  });

  it("does not refresh when the active channel is not the voice session channel", async () => {
    const refreshVoiceSessionPermissions = vi.fn(async () => undefined);
    const [session] = createSignal(SESSION);
    const [isVoiceSessionActive] = createSignal(true);
    const [isVoiceSessionForActiveChannel] = createSignal(false);
    const [voiceSessionChannelKey] = createSignal("guild-a|channel-a");
    const [isJoiningVoice] = createSignal(false);
    const [isLeavingVoice] = createSignal(false);
    const [canPublishVoiceCamera] = createSignal(false);
    const [canPublishVoiceScreenShare] = createSignal(true);
    const [canSubscribeVoiceStreams] = createSignal(true);
    const [voiceSessionCapabilities] = createSignal({
      canSubscribe: false,
      publishSources: ["microphone"] as ("microphone" | "camera" | "screen_share")[],
    });

    const dispose = createRoot((innerDispose) => {
      createVoicePermissionSyncController({
        session,
        isVoiceSessionActive,
        isVoiceSessionForActiveChannel,
        voiceSessionChannelKey,
        isJoiningVoice,
        isLeavingVoice,
        canPublishVoiceCamera,
        canPublishVoiceScreenShare,
        canSubscribeVoiceStreams,
        voiceSessionCapabilities,
        refreshVoiceSessionPermissions,
      });
      return innerDispose;
    });

    await flush();
    expect(refreshVoiceSessionPermissions).not.toHaveBeenCalled();
    dispose();
  });

  it("derives desired voice permission state and mismatch checks deterministically", () => {
    const desired = resolveDesiredVoicePermissionState({
      canPublishVoiceCamera: () => true,
      canPublishVoiceScreenShare: () => false,
      canSubscribeVoiceStreams: () => true,
    });
    expect(desired).toEqual({
      canSubscribe: true,
      canPublishCamera: true,
      canPublishScreenShare: false,
    });
    expect(
      hasVoiceSessionPermissionMismatch(desired, {
        canSubscribe: true,
        publishSources: ["microphone", "camera"],
      }),
    ).toBe(false);
    expect(
      hasVoiceSessionPermissionMismatch(desired, {
        canSubscribe: false,
        publishSources: ["microphone", "camera", "screen_share"],
      }),
    ).toBe(true);
  });
});
