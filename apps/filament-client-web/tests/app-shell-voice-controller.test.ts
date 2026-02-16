import { createMemo, createRoot, createSignal, type Setter } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelFromResponse,
  guildIdFromInput,
  voiceTokenFromResponse,
  type ChannelRecord,
} from "../src/domain/chat";
import { RTC_DISCONNECTED_SNAPSHOT } from "../src/features/app-shell/config/ui-constants";
import {
  createVoiceOperationsController,
  type VoiceOperationsControllerDependencies,
} from "../src/features/app-shell/controllers/voice-operations-controller";
import {
  resolveVoiceConnectionTransition,
  resolveVoiceDevicePreferenceStatus,
  unavailableVoiceDeviceError,
} from "../src/features/app-shell/controllers/voice-controller";
import {
  DEFAULT_VOICE_SESSION_CAPABILITIES,
} from "../src/features/app-shell/state/voice-state";
import type { AsyncOperationState } from "../src/features/app-shell/state/async-operation-state";
import { mediaDeviceIdFromInput } from "../src/lib/voice-device-settings";
import type { RtcClient, RtcSnapshot } from "../src/lib/rtc";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const VOICE_CHANNEL = channelFromResponse({
  channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
  name: "bridge",
  kind: "voice",
});

const TEXT_CHANNEL = channelFromResponse({
  channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
  name: "general",
  kind: "text",
});

function createRtcClientMock(overrides: Partial<RtcClient> = {}): RtcClient {
  return {
    snapshot: () => RTC_DISCONNECTED_SNAPSHOT,
    subscribe: () => () => {},
    setAudioInputDevice: async () => {},
    setAudioOutputDevice: async () => {},
    join: async () => {},
    leave: async () => {},
    setMicrophoneEnabled: async () => {},
    toggleMicrophone: async () => false,
    setCameraEnabled: async () => {},
    toggleCamera: async () => false,
    setScreenShareEnabled: async () => {},
    toggleScreenShare: async () => false,
    destroy: async () => {},
    ...overrides,
  };
}

function deferred<T>() {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((reason?: unknown) => void) | undefined;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
    reject: (reason?: unknown) => reject?.(reason),
  };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function createVoiceOperationsHarness(input?: {
  dependencies?: Partial<VoiceOperationsControllerDependencies>;
  activeChannel?: ChannelRecord | null;
  canPublishVoiceCamera?: boolean;
  canPublishVoiceScreenShare?: boolean;
  canSubscribeVoiceStreams?: boolean;
  canToggleVoiceCamera?: boolean;
  canToggleVoiceScreenShare?: boolean;
  audioInputDeviceId?: string | null;
  audioOutputDeviceId?: string | null;
  initialVoiceJoinState?: AsyncOperationState;
  initialVoiceSessionChannelKey?: string | null;
}) {
  const [session] = createSignal(SESSION);
  const [activeGuildId] = createSignal(GUILD_ID);
  const [activeChannel] = createSignal(input?.activeChannel ?? VOICE_CHANNEL);
  const [canPublishVoiceCamera] = createSignal(input?.canPublishVoiceCamera ?? false);
  const [canPublishVoiceScreenShare] = createSignal(input?.canPublishVoiceScreenShare ?? false);
  const [canSubscribeVoiceStreams] = createSignal(input?.canSubscribeVoiceStreams ?? true);
  const [canToggleVoiceCamera] = createSignal(input?.canToggleVoiceCamera ?? true);
  const [canToggleVoiceScreenShare] = createSignal(input?.canToggleVoiceScreenShare ?? true);

  const [isLeavingVoice, setLeavingVoice] = createSignal(false);
  const [isTogglingVoiceMic, setTogglingVoiceMic] = createSignal(false);
  const [isTogglingVoiceCamera, setTogglingVoiceCamera] = createSignal(false);
  const [isTogglingVoiceScreenShare, setTogglingVoiceScreenShare] = createSignal(false);

  const [voiceStatus, setVoiceStatus] = createSignal("");
  const [voiceError, setVoiceError] = createSignal("");
  const [voiceJoinState, setVoiceJoinState] = createSignal<AsyncOperationState>(
    input?.initialVoiceJoinState ?? {
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    },
  );
  const voiceJoinPhaseTransitions: AsyncOperationState["phase"][] = [];
  const setVoiceJoinStateTracked: Setter<AsyncOperationState> = (value) => {
    const resolveNext = (previous: AsyncOperationState): AsyncOperationState =>
      typeof value === "function"
        ? (value as (previous: AsyncOperationState) => AsyncOperationState)(previous)
        : value;
    return setVoiceJoinState((previous) => {
      const next = resolveNext(previous);
      voiceJoinPhaseTransitions.push(next.phase);
      return next;
    });
  };
  const isJoiningVoice = createMemo(
    () => voiceJoinState().phase === "running",
  );
  const [audioDevicesError, setAudioDevicesError] = createSignal("");
  const [rtcSnapshot, setRtcSnapshot] = createSignal<RtcSnapshot>(RTC_DISCONNECTED_SNAPSHOT);
  const [voiceSessionChannelKey, setVoiceSessionChannelKey] = createSignal<string | null>(
    input?.initialVoiceSessionChannelKey ?? "seeded|channel",
  );
  const [voiceSessionStartedAtUnixMs, setVoiceSessionStartedAtUnixMs] = createSignal<number | null>(
    111,
  );
  const [voiceDurationClockUnixMs, setVoiceDurationClockUnixMs] = createSignal(0);
  const [voiceSessionCapabilities, setVoiceSessionCapabilities] = createSignal({
    canSubscribe: true,
    publishSources: ["microphone"] as ("microphone" | "camera" | "screen_share")[],
  });

  const [voiceDevicePreferences] = createSignal({
    audioInputDeviceId:
      input?.audioInputDeviceId === null
        ? null
        : mediaDeviceIdFromInput(input?.audioInputDeviceId ?? "mic-pref-1"),
    audioOutputDeviceId:
      input?.audioOutputDeviceId === null
        ? null
        : mediaDeviceIdFromInput(input?.audioOutputDeviceId ?? "spk-pref-1"),
  });

  const controller = createVoiceOperationsController(
    {
      session,
      activeGuildId,
      activeChannel,
      voiceSessionChannelKey,
      canPublishVoiceCamera,
      canPublishVoiceScreenShare,
      canSubscribeVoiceStreams,
      canToggleVoiceCamera,
      canToggleVoiceScreenShare,
      isJoiningVoice,
      isLeavingVoice,
      isTogglingVoiceMic,
      isTogglingVoiceCamera,
      isTogglingVoiceScreenShare,
      voiceDevicePreferences,
      setRtcSnapshot,
      setVoiceStatus,
      setVoiceError,
      setVoiceJoinState: setVoiceJoinStateTracked,
      setLeavingVoice,
      setTogglingVoiceMic,
      setTogglingVoiceCamera,
      setTogglingVoiceScreenShare,
      setVoiceSessionChannelKey,
      setVoiceSessionStartedAtUnixMs,
      setVoiceDurationClockUnixMs,
      setVoiceSessionCapabilities,
      setAudioDevicesError,
      defaultVoiceSessionCapabilities: DEFAULT_VOICE_SESSION_CAPABILITIES,
    },
    input?.dependencies,
  );

  return {
    controller,
    voiceStatus,
    voiceError,
    voiceJoinState,
    voiceJoinPhaseTransitions: () => [...voiceJoinPhaseTransitions],
    audioDevicesError,
    rtcSnapshot,
    voiceSessionChannelKey,
    voiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs,
    voiceSessionCapabilities,
  };
}

describe("app shell voice controller", () => {
  it("emits reconnecting and reconnected transitions", () => {
    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "connected",
        currentStatus: "reconnecting",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: false,
      statusMessage: "Voice reconnecting. Media may recover automatically.",
      errorMessage: "",
    });

    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "reconnecting",
        currentStatus: "connected",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: false,
      statusMessage: "Voice reconnected.",
      errorMessage: "",
    });
  });

  it("forces voice session clear on unexpected disconnect", () => {
    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "connected",
        currentStatus: "disconnected",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: true,
      statusMessage: "",
      errorMessage: "Voice connection dropped. Select Join Voice to reconnect.",
    });
  });

  it("maps voice device status strings for active and inactive sessions", () => {
    expect(resolveVoiceDevicePreferenceStatus("audioinput", false, "mic-1")).toBe(
      "Microphone preference saved for the next voice join.",
    );
    expect(resolveVoiceDevicePreferenceStatus("audiooutput", true, "spk-1")).toBe(
      "Speaker updated for the active voice session.",
    );
    expect(resolveVoiceDevicePreferenceStatus("audioinput", true, null)).toBe(
      "Microphone preference cleared. Current session keeps its current device.",
    );
  });

  it("returns unavailable-device errors by device kind", () => {
    expect(unavailableVoiceDeviceError("audioinput")).toBe(
      "Selected microphone is not available.",
    );
    expect(unavailableVoiceDeviceError("audiooutput")).toBe(
      "Selected speaker is not available.",
    );
  });

  it("releases rtc client with deterministic state reset when destroy fails", async () => {
    const unsubscribe = vi.fn();
    const subscribe = vi.fn(() => unsubscribe);
    const destroy = vi.fn(async () => {
      throw new Error("destroy_failed");
    });
    const createRtcClientMockDependency = vi.fn(() =>
      createRtcClientMock({
        subscribe,
        destroy,
      }),
    );

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        dependencies: {
          createRtcClient: createRtcClientMockDependency,
        },
      }),
    );

    harness.controller.ensureRtcClient();
    await harness.controller.releaseRtcClient();

    expect(createRtcClientMockDependency).toHaveBeenCalledTimes(1);
    expect(subscribe).toHaveBeenCalledTimes(1);
    expect(unsubscribe).toHaveBeenCalledTimes(1);
    expect(destroy).toHaveBeenCalledTimes(1);
    expect(harness.rtcSnapshot()).toEqual(RTC_DISCONNECTED_SNAPSHOT);
    expect(harness.voiceSessionChannelKey()).toBeNull();
    expect(harness.voiceSessionStartedAtUnixMs()).toBeNull();
    expect(harness.voiceSessionCapabilities()).toEqual(DEFAULT_VOICE_SESSION_CAPABILITIES);
  });

  it("blocks camera and screen-share toggles when publish sources are denied", async () => {
    const toggleCamera = vi.fn(async () => true);
    const toggleScreenShare = vi.fn(async () => true);

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        canToggleVoiceCamera: false,
        canToggleVoiceScreenShare: false,
        dependencies: {
          createRtcClient: () =>
            createRtcClientMock({
              toggleCamera,
              toggleScreenShare,
            }),
        },
      }),
    );

    harness.controller.ensureRtcClient();
    await harness.controller.toggleVoiceCamera();
    expect(toggleCamera).not.toHaveBeenCalled();
    expect(harness.voiceError()).toBe("Camera publish is not allowed for this call.");

    await harness.controller.toggleVoiceScreenShare();
    expect(toggleScreenShare).not.toHaveBeenCalled();
    expect(harness.voiceError()).toBe("Screen share publish is not allowed for this call.");
  });

  it("applies device preferences before join and keeps token request least-privilege", async () => {
    const issueVoiceTokenMock = vi.fn(async () =>
      voiceTokenFromResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.123",
        can_publish: true,
        can_subscribe: false,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      }),
    );
    const setAudioInputDevice = vi.fn(async (_deviceId: string | null) => {});
    const setAudioOutputDevice = vi.fn(async (_deviceId: string | null) => {});
    const join = vi.fn(async () => {});
    const setMicrophoneEnabled = vi.fn(async (_enabled: boolean) => {});

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        canPublishVoiceCamera: false,
        canPublishVoiceScreenShare: true,
        canSubscribeVoiceStreams: false,
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
          createRtcClient: () =>
            createRtcClientMock({
              setAudioInputDevice,
              setAudioOutputDevice,
              join,
              setMicrophoneEnabled,
            }),
          now: () => 4242,
        },
      }),
    );

    await harness.controller.joinVoiceChannel();

    expect(issueVoiceTokenMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      VOICE_CHANNEL.channelId,
      {
        canSubscribe: false,
        publishSources: ["microphone", "screen_share"],
      },
    );
    expect(setAudioInputDevice).toHaveBeenCalledWith(mediaDeviceIdFromInput("mic-pref-1"));
    expect(setAudioOutputDevice).toHaveBeenCalledWith(mediaDeviceIdFromInput("spk-pref-1"));
    expect(join).toHaveBeenCalledTimes(1);
    expect(setMicrophoneEnabled).toHaveBeenCalledWith(true);
    expect(harness.voiceSessionChannelKey()).toBe(`${GUILD_ID}|${VOICE_CHANNEL.channelId}`);
    expect(harness.voiceSessionStartedAtUnixMs()).toBe(4242);
    expect(harness.voiceDurationClockUnixMs()).toBe(4242);
    expect(harness.voiceStatus()).toBe("Voice connected. Microphone enabled.");
    expect(harness.voiceJoinState()).toEqual({
      phase: "succeeded",
      statusMessage: "Voice connected. Microphone enabled.",
      errorMessage: "",
    });
    expect(harness.audioDevicesError()).toBe("");
  });

  it("records failed join state when token issuance fails", async () => {
    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        dependencies: {
          issueVoiceToken: async () => {
            throw new Error("denied");
          },
        },
      }),
    );

    await harness.controller.joinVoiceChannel();

    expect(harness.voiceJoinState()).toEqual({
      phase: "failed",
      statusMessage: "",
      errorMessage: "Unable to join voice.",
    });
    expect(harness.voiceStatus()).toBe("");
    expect(harness.voiceError()).toBe("Unable to join voice.");
  });

  it("keeps succeeded join state when microphone activation fails while populating voice error", async () => {
    const issueVoiceTokenMock = vi.fn(async () =>
      voiceTokenFromResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.456",
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      }),
    );
    const setMicrophoneEnabled = vi.fn(async () => {
      throw new Error("mic_failed");
    });

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
          createRtcClient: () =>
            createRtcClientMock({
              setMicrophoneEnabled,
            }),
        },
      }),
    );

    await harness.controller.joinVoiceChannel();

    expect(setMicrophoneEnabled).toHaveBeenCalledWith(true);
    expect(harness.voiceJoinState()).toEqual({
      phase: "succeeded",
      statusMessage: "Voice connected.",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("Voice connected.");
    expect(harness.voiceError()).toBe("Connected, but microphone activation failed.");
  });

  it("clears stale voice error when a later join succeeds", async () => {
    let joinAttempt = 0;
    const issueVoiceTokenMock = vi.fn(async () => {
      joinAttempt += 1;
      if (joinAttempt === 1) {
        throw new Error("denied");
      }
      return voiceTokenFromResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.789",
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      });
    });

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
          createRtcClient: () => createRtcClientMock(),
        },
      }),
    );

    await harness.controller.joinVoiceChannel();
    expect(harness.voiceJoinState().phase).toBe("failed");
    expect(harness.voiceError()).toBe("Unable to join voice.");

    await harness.controller.joinVoiceChannel();
    expect(harness.voiceJoinState()).toEqual({
      phase: "succeeded",
      statusMessage: "Voice connected. Microphone enabled.",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("Voice connected. Microphone enabled.");
    expect(harness.voiceError()).toBe("");
  });

  it("resets stale voice join status and error when join preconditions are not met", async () => {
    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        activeChannel: TEXT_CHANNEL,
        initialVoiceJoinState: {
          phase: "failed",
          statusMessage: "Voice connected.",
          errorMessage: "Unable to join voice.",
        },
      }),
    );

    await harness.controller.joinVoiceChannel();

    expect(harness.voiceJoinState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("");
    expect(harness.voiceError()).toBe("");
    expect(harness.voiceJoinPhaseTransitions()).toEqual(["idle"]);
  });

  it("resets voice join state when leaving voice session", async () => {
    const leave = vi.fn(async () => {});
    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        initialVoiceJoinState: {
          phase: "failed",
          statusMessage: "",
          errorMessage: "Unable to join voice.",
        },
        dependencies: {
          createRtcClient: () =>
            createRtcClientMock({
              leave,
            }),
        },
      }),
    );

    harness.controller.ensureRtcClient();
    await harness.controller.leaveVoiceChannel("Left voice channel.");

    expect(leave).toHaveBeenCalledTimes(1);
    expect(harness.voiceJoinState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("Left voice channel.");
    expect(harness.voiceError()).toBe("");
  });

  it("keeps running join state on duplicate join attempts", async () => {
    const issueVoiceTokenGate = deferred<ReturnType<typeof voiceTokenFromResponse>>();
    const issueVoiceTokenMock = vi.fn(() => issueVoiceTokenGate.promise);
    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
        },
      }),
    );

    const firstJoin = harness.controller.joinVoiceChannel();
    await flush();
    expect(harness.voiceJoinState().phase).toBe("running");

    await harness.controller.joinVoiceChannel();
    expect(issueVoiceTokenMock).toHaveBeenCalledTimes(1);
    expect(harness.voiceJoinState().phase).toBe("running");
    expect(harness.voiceJoinPhaseTransitions()).toEqual(["running"]);

    issueVoiceTokenGate.reject(new Error("denied"));
    await expect(firstJoin).resolves.toBeUndefined();
    expect(harness.voiceJoinState()).toEqual({
      phase: "failed",
      statusMessage: "",
      errorMessage: "Unable to join voice.",
    });
    expect(harness.voiceJoinPhaseTransitions()).toEqual(["running", "failed"]);
  });

  it("does not request a new token when already connected to the same voice channel", async () => {
    const issueVoiceTokenMock = vi.fn(async () =>
      voiceTokenFromResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.idempotent",
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      }),
    );

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        initialVoiceSessionChannelKey: `${GUILD_ID}|${VOICE_CHANNEL.channelId}`,
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
        },
      }),
    );

    harness.controller.ensureRtcClient();
    await harness.controller.joinVoiceChannel();

    expect(issueVoiceTokenMock).not.toHaveBeenCalled();
    expect(harness.voiceJoinState()).toEqual({
      phase: "succeeded",
      statusMessage: "Voice connected.",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("Voice connected.");
    expect(harness.voiceError()).toBe("");
  });

  it("clears stale failed join state during same-channel idempotent join", async () => {
    const issueVoiceTokenMock = vi.fn(async () =>
      voiceTokenFromResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.stale",
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      }),
    );

    const harness = createRoot(() =>
      createVoiceOperationsHarness({
        initialVoiceJoinState: {
          phase: "failed",
          statusMessage: "",
          errorMessage: "Unable to join voice.",
        },
        initialVoiceSessionChannelKey: `${GUILD_ID}|${VOICE_CHANNEL.channelId}`,
        dependencies: {
          issueVoiceToken: issueVoiceTokenMock,
        },
      }),
    );

    await harness.controller.joinVoiceChannel();

    expect(issueVoiceTokenMock).not.toHaveBeenCalled();
    expect(harness.voiceJoinState()).toEqual({
      phase: "succeeded",
      statusMessage: "Voice connected.",
      errorMessage: "",
    });
    expect(harness.voiceStatus()).toBe("Voice connected.");
    expect(harness.voiceError()).toBe("");
  });
});
