import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";
import { RtcClientError } from "../src/lib/rtc";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";
const VOICE_SETTINGS_KEY = "filament.voice.settings.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const TEXT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FB0";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const REMOTE_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FB1";

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

type FixtureChannel = {
  channelId: string;
  name: string;
  kind: "text" | "voice";
};
const DEFAULT_CHANNELS: FixtureChannel[] = [{ channelId: CHANNEL_ID, name: "bridge", kind: "voice" }];

const rtcMock = vi.hoisted(() => {
  type MockParticipant = { identity: string; subscribedTrackCount: number };
  type MockVideoTrack = {
    trackSid: string;
    participantIdentity: string;
    source: "camera" | "screen_share";
    isLocal: boolean;
  };
  type MockSnapshot = {
    connectionStatus: "disconnected" | "connecting" | "connected" | "reconnecting" | "error";
    localParticipantIdentity: string | null;
    isMicrophoneEnabled: boolean;
    isDeafened: boolean;
    isCameraEnabled: boolean;
    isScreenShareEnabled: boolean;
    participants: MockParticipant[];
    videoTracks: MockVideoTrack[];
    activeSpeakerIdentities: string[];
    lastErrorCode: string | null;
    lastErrorMessage: string | null;
  };

  const createDisconnectedSnapshot = (): MockSnapshot => ({
    connectionStatus: "disconnected",
    localParticipantIdentity: null,
    isMicrophoneEnabled: false,
    isDeafened: false,
    isCameraEnabled: false,
    isScreenShareEnabled: false,
    participants: [],
    videoTracks: [],
    activeSpeakerIdentities: [],
    lastErrorCode: null,
    lastErrorMessage: null,
  });

  const listeners = new Set<(snapshot: MockSnapshot) => void>();
  let snapshot = createDisconnectedSnapshot();
  let joinParticipants: MockParticipant[] = [];
  let joinFailure: Error | null = null;
  let leaveFailure: Error | null = null;
  let destroyFailure: Error | null = null;
  let localParticipantIdentity = "u.local";

  const cloneParticipants = (participants: MockParticipant[]): MockParticipant[] =>
    participants.map((entry) => ({
      identity: entry.identity,
      subscribedTrackCount: entry.subscribedTrackCount,
    }));
  const cloneVideoTracks = (tracks: MockVideoTrack[]): MockVideoTrack[] =>
    tracks.map((track) => ({ ...track }));

  const upsertLocalVideoTrack = (
    tracks: MockVideoTrack[],
    nextTrack: MockVideoTrack,
  ): MockVideoTrack[] => {
    const filtered = tracks.filter(
      (track) =>
        !(
          track.isLocal &&
          track.source === nextTrack.source &&
          track.participantIdentity === nextTrack.participantIdentity
        ),
    );
    return [...filtered, nextTrack];
  };

  const emit = () => {
    const next = {
      ...snapshot,
      participants: cloneParticipants(snapshot.participants),
      videoTracks: cloneVideoTracks(snapshot.videoTracks),
    };
    for (const listener of listeners) {
      listener(next);
    }
  };

  const join = vi.fn(async () => {
    if (joinFailure) {
      const coded = joinFailure as Error & { code?: unknown };
      const errorCode = typeof coded.code === "string" ? coded.code : "join_failed";
      snapshot = {
        ...snapshot,
        connectionStatus: "error",
        lastErrorCode: errorCode,
        lastErrorMessage: joinFailure.message,
      };
      emit();
      throw joinFailure;
    }
    snapshot = {
      ...snapshot,
      connectionStatus: "connected",
      localParticipantIdentity,
      participants: cloneParticipants(joinParticipants),
      activeSpeakerIdentities: [],
      lastErrorCode: null,
      lastErrorMessage: null,
    };
    emit();
  });

  const leave = vi.fn(async () => {
    if (leaveFailure) {
      throw leaveFailure;
    }
    snapshot = createDisconnectedSnapshot();
    emit();
  });

  const setMicrophoneEnabled = vi.fn(async (enabled: boolean) => {
    snapshot = {
      ...snapshot,
      isMicrophoneEnabled: enabled,
    };
    emit();
  });

  const toggleMicrophone = vi.fn(async () => {
    snapshot = {
      ...snapshot,
      isMicrophoneEnabled: !snapshot.isMicrophoneEnabled,
    };
    emit();
    return snapshot.isMicrophoneEnabled;
  });

  const setDeafened = vi.fn(async (enabled: boolean) => {
    snapshot = {
      ...snapshot,
      isDeafened: enabled,
    };
    emit();
  });

  const toggleDeafened = vi.fn(async () => {
    snapshot = {
      ...snapshot,
      isDeafened: !snapshot.isDeafened,
    };
    emit();
    return snapshot.isDeafened;
  });

  const setCameraEnabled = vi.fn(async (enabled: boolean) => {
    snapshot = {
      ...snapshot,
      isCameraEnabled: enabled,
      videoTracks: enabled
        ? upsertLocalVideoTrack(snapshot.videoTracks, {
          trackSid: "L-CAMERA",
          participantIdentity: "u.local",
          source: "camera",
          isLocal: true,
        })
        : snapshot.videoTracks.filter((track) => !(track.isLocal && track.source === "camera")),
    };
    emit();
  });

  const toggleCamera = vi.fn(async () => {
    const enabled = !snapshot.isCameraEnabled;
    await setCameraEnabled(enabled);
    return enabled;
  });

  const setScreenShareEnabled = vi.fn(async (enabled: boolean) => {
    snapshot = {
      ...snapshot,
      isScreenShareEnabled: enabled,
      videoTracks: enabled
        ? upsertLocalVideoTrack(snapshot.videoTracks, {
          trackSid: "L-SCREEN",
          participantIdentity: "u.local",
          source: "screen_share",
          isLocal: true,
        })
        : snapshot.videoTracks.filter(
          (track) => !(track.isLocal && track.source === "screen_share"),
        ),
    };
    emit();
  });

  const toggleScreenShare = vi.fn(async () => {
    const enabled = !snapshot.isScreenShareEnabled;
    await setScreenShareEnabled(enabled);
    return enabled;
  });

  const destroy = vi.fn(async () => {
    if (destroyFailure) {
      throw destroyFailure;
    }
    await leave();
    listeners.clear();
  });

  const setAudioInputDevice = vi.fn(async (_deviceId: string | null) => { });
  const setAudioOutputDevice = vi.fn(async (_deviceId: string | null) => { });
  const attachVideoTrack = vi.fn((_trackSid: string, _element: HTMLVideoElement) => { });
  const detachVideoTrack = vi.fn((_trackSid: string, _element: HTMLVideoElement) => { });

  const client = {
    snapshot: () => snapshot,
    subscribe(listener: (nextSnapshot: MockSnapshot) => void) {
      listeners.add(listener);
      listener({
        ...snapshot,
        participants: cloneParticipants(snapshot.participants),
        videoTracks: cloneVideoTracks(snapshot.videoTracks),
      });
      return () => {
        listeners.delete(listener);
      };
    },
    join,
    leave,
    setAudioInputDevice,
    setAudioOutputDevice,
    setMicrophoneEnabled,
    toggleMicrophone,
    setDeafened,
    toggleDeafened,
    setCameraEnabled,
    toggleCamera,
    setScreenShareEnabled,
    toggleScreenShare,
    attachVideoTrack,
    detachVideoTrack,
    destroy,
  };

  const createRtcClient = vi.fn(() => client);

  const setJoinParticipants = (participants: MockParticipant[]) => {
    joinParticipants = cloneParticipants(participants);
  };

  const setActiveSpeakerIdentities = (identities: string[]) => {
    snapshot = {
      ...snapshot,
      activeSpeakerIdentities: [...new Set(identities)],
    };
    emit();
  };

  const setVideoTracks = (tracks: MockVideoTrack[]) => {
    snapshot = {
      ...snapshot,
      videoTracks: cloneVideoTracks(tracks),
      isCameraEnabled: tracks.some((track) => track.isLocal && track.source === "camera"),
      isScreenShareEnabled: tracks.some((track) => track.isLocal && track.source === "screen_share"),
    };
    emit();
  };

  const setConnectionStatus = (
    status: MockSnapshot["connectionStatus"],
    options?: { lastErrorCode?: string | null; lastErrorMessage?: string | null },
  ) => {
    snapshot = {
      ...snapshot,
      connectionStatus: status,
      lastErrorCode:
        options?.lastErrorCode !== undefined
          ? options.lastErrorCode
          : status === "error"
            ? snapshot.lastErrorCode
            : null,
      lastErrorMessage:
        options?.lastErrorMessage !== undefined
          ? options.lastErrorMessage
          : status === "error"
            ? snapshot.lastErrorMessage
            : null,
    };
    emit();
  };

  const setJoinFailure = (error: Error | null) => {
    joinFailure = error;
  };

  const setLeaveFailure = (error: Error | null) => {
    leaveFailure = error;
  };

  const setDestroyFailure = (error: Error | null) => {
    destroyFailure = error;
  };

  const setLocalParticipantIdentity = (identity: string) => {
    localParticipantIdentity = identity;
  };

  const reset = () => {
    listeners.clear();
    snapshot = createDisconnectedSnapshot();
    joinParticipants = [];
    joinFailure = null;
    leaveFailure = null;
    destroyFailure = null;
    localParticipantIdentity = "u.local";
    createRtcClient.mockClear();
    join.mockClear();
    leave.mockClear();
    setAudioInputDevice.mockClear();
    setAudioOutputDevice.mockClear();
    setMicrophoneEnabled.mockClear();
    toggleMicrophone.mockClear();
    setDeafened.mockClear();
    toggleDeafened.mockClear();
    setCameraEnabled.mockClear();
    toggleCamera.mockClear();
    setScreenShareEnabled.mockClear();
    toggleScreenShare.mockClear();
    destroy.mockClear();
  };

  return {
    createRtcClient,
    join,
    leave,
    setAudioInputDevice,
    setAudioOutputDevice,
    setMicrophoneEnabled,
    toggleMicrophone,
    setDeafened,
    toggleDeafened,
    setCameraEnabled,
    toggleCamera,
    setScreenShareEnabled,
    toggleScreenShare,
    destroy,
    setJoinParticipants,
    setActiveSpeakerIdentities,
    setVideoTracks,
    setConnectionStatus,
    setJoinFailure,
    setLeaveFailure,
    setDestroyFailure,
    setLocalParticipantIdentity,
    reset,
  };
});

vi.mock("../src/lib/rtc", () => {
  class MockRtcClientError extends Error {
    readonly code: string;

    constructor(code: string, message: string) {
      super(message);
      this.name = "RtcClientError";
      this.code = code;
    }
  }

  return {
    createRtcClient: rtcMock.createRtcClient,
    RtcClientError: MockRtcClientError,
  };
});

function createStorageMock(): Storage {
  const store = new Map<string, string>();

  return {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    key(index: number) {
      return [...store.keys()][index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, value);
    },
  };
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
}

function requestMethod(init?: RequestInit): string {
  return (init?.method ?? "GET").toUpperCase();
}

function extractChannelId(url: string): string | null {
  const match = url.match(new RegExp(`/guilds/${GUILD_ID}/channels/([^/?]+)`));
  return match?.[1] ?? null;
}

function seedAuthenticatedWorkspace(channels: FixtureChannel[] = DEFAULT_CHANNELS): void {
  window.sessionStorage.setItem(
    SESSION_STORAGE_KEY,
    JSON.stringify({
      accessToken: ACCESS_TOKEN,
      refreshToken: REFRESH_TOKEN,
      expiresAtUnix: Math.floor(Date.now() / 1000) + 3600,
    }),
  );

  window.localStorage.setItem(
    WORKSPACE_CACHE_KEY,
    JSON.stringify([
      {
        guildId: GUILD_ID,
        guildName: "Security Ops",
        channels,
      },
    ]),
  );
}

function createVoiceFixtureFetch(options?: {
  channels?: FixtureChannel[];
  voicePermissions?: string[];
  userLookupById?: Record<string, string>;
  voiceTokenDelay?: Promise<void>;
  voiceTokenResponse?: {
    can_publish?: boolean;
    can_subscribe?: boolean;
    publish_sources?: string[];
  };
  voiceTokenError?: {
    status: number;
    code: string;
  };
}) {
  const channels = options?.channels ?? DEFAULT_CHANNELS;
  const voicePermissions = options?.voicePermissions ?? ["create_message", "subscribe_streams"];
  const voiceTokenResponse = options?.voiceTokenResponse ?? {
    can_publish: true,
    can_subscribe: true,
    publish_sources: ["microphone"],
  };
  const channelsById = new Map(channels.map((channel) => [channel.channelId, channel]));
  const userLookupById = options?.userLookupById ?? {};
  let voiceTokenBody: unknown = null;
  let voiceTokenRequestCount = 0;
  const voiceStateBodies: unknown[] = [];
  const userLookupBodies: unknown[] = [];

  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = requestUrl(input);
    const method = requestMethod(init);

    if (method === "GET" && url.includes("/auth/me")) {
      return jsonResponse({ user_id: USER_ID, username: "alice" });
    }

    if (method === "GET" && url.endsWith("/guilds")) {
      return jsonResponse({
        guilds: [{ guild_id: GUILD_ID, name: "Security Ops", visibility: "private" }],
      });
    }

    if (method === "GET" && url.endsWith(`/guilds/${GUILD_ID}/channels`)) {
      return jsonResponse({
        channels: channels.map((channel) => ({
          channel_id: channel.channelId,
          name: channel.name,
          kind: channel.kind,
        })),
      });
    }

    const channelId = extractChannelId(url);
    const channel = channelId ? channelsById.get(channelId) : undefined;

    if (
      method === "GET" &&
      channel &&
      url.includes(`/guilds/${GUILD_ID}/channels/${channel.channelId}/permissions/self`)
    ) {
      return jsonResponse({
        role: "member",
        permissions:
          channel.kind === "voice"
            ? voicePermissions
            : ["create_message"],
      });
    }

    if (
      method === "GET" &&
      channel &&
      url.includes(`/guilds/${GUILD_ID}/channels/${channel.channelId}/messages`)
    ) {
      return jsonResponse({ messages: [], next_before: null });
    }

    if (method === "GET" && url.includes("/guilds/public")) {
      return jsonResponse({ guilds: [] });
    }

    if (method === "GET" && url.endsWith("/friends")) {
      return jsonResponse({ friends: [] });
    }

    if (method === "GET" && url.endsWith("/friends/requests")) {
      return jsonResponse({ incoming: [], outgoing: [] });
    }

    if (method === "POST" && url.endsWith("/users/lookup")) {
      const body = init?.body ? JSON.parse(init.body as string) : {};
      userLookupBodies.push(body);
      const userIds = Array.isArray(body.user_ids) ? body.user_ids : [];
      const users = userIds.flatMap((userId: unknown) => {
        if (typeof userId !== "string") {
          return [];
        }
        const username = userLookupById[userId];
        if (!username) {
          return [];
        }
        return [{ user_id: userId, username }];
      });
      return jsonResponse({ users });
    }

    if (
      method === "POST" &&
      channel?.kind === "voice" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${channel.channelId}/voice/token`)
    ) {
      voiceTokenRequestCount += 1;
      voiceTokenBody = init?.body ? JSON.parse(init.body as string) : null;
      if (options?.voiceTokenDelay) {
        await options.voiceTokenDelay;
      }
      if (options?.voiceTokenError) {
        return jsonResponse({ error: options.voiceTokenError.code }, options.voiceTokenError.status);
      }
      return jsonResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.123",
        can_publish: voiceTokenResponse.can_publish ?? true,
        can_subscribe: voiceTokenResponse.can_subscribe ?? true,
        publish_sources: voiceTokenResponse.publish_sources ?? ["microphone"],
        expires_in_secs: 300,
      });
    }

    if (
      method === "POST" &&
      channel?.kind === "voice" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${channel.channelId}/voice/state`)
    ) {
      voiceStateBodies.push(init?.body ? JSON.parse(init.body as string) : null);
      return new Response(null, { status: 204 });
    }

    if (method === "POST" && url.endsWith("/auth/logout")) {
      return new Response(null, { status: 204 });
    }

    return jsonResponse({ error: "not_found" }, 404);
  });

  return {
    fetchMock,
    voiceTokenBody: () => voiceTokenBody,
    voiceTokenRequestCount: () => voiceTokenRequestCount,
    voiceStateBodies: () => voiceStateBodies,
    userLookupBodies: () => userLookupBodies,
  };
}

const findVoiceControl = async (name: string | RegExp) => {
  const controls = await screen.findAllByRole("button", { name });
  return controls[0];
};

const getVoiceControl = (name: string | RegExp) => {
  const controls = screen.getAllByRole("button", { name });
  return controls[0];
};

describe("app shell voice controls", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    const storage = createStorageMock();
    vi.stubGlobal("localStorage", storage);
    Object.defineProperty(window, "localStorage", {
      value: storage,
      configurable: true,
    });
    rtcMock.reset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("joins a voice channel from header controls with voice-first token payload", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    const joinButton = await screen.findByRole("button", { name: "Join Voice" });
    fireEvent.click(joinButton);

    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(rtcMock.setMicrophoneEnabled).toHaveBeenCalledWith(true);
    expect(fixture.voiceTokenBody()).toEqual({
      can_subscribe: true,
      publish_sources: ["microphone"],
    });

    expect(await findVoiceControl("Mute Mic")).toBeInTheDocument();
  });

  it("resolves stream control icons from the app resource directory", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    const muteButton = await findVoiceControl("Mute Mic");
    const muteIcon = muteButton.querySelector(".icon-mask");
    expect(muteIcon).toBeTruthy();

    const iconStyle = muteIcon?.getAttribute("style") ?? "";
    expect(iconStyle).toContain("data:image/svg+xml");
    expect(iconStyle).not.toContain("/src/resource/");
  });

  it("requests publish sources for camera/screen when stream permissions allow them", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      voicePermissions: [
        "create_message",
        "subscribe_streams",
        "publish_video",
        "publish_screen_share",
      ],
      voiceTokenResponse: {
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone", "camera", "screen_share"],
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(fixture.voiceTokenBody()).toEqual({
      can_subscribe: true,
      publish_sources: ["microphone", "camera", "screen_share"],
    });
  });

  it("wires saved audio device preferences into RTC before join", async () => {
    seedAuthenticatedWorkspace();
    window.localStorage.setItem(
      VOICE_SETTINGS_KEY,
      JSON.stringify({
        audioInputDeviceId: "mic-pref-1",
        audioOutputDeviceId: "spk-pref-1",
      }),
    );
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(rtcMock.setAudioInputDevice).toHaveBeenCalledWith("mic-pref-1");
    expect(rtcMock.setAudioOutputDevice).toHaveBeenCalledWith("spk-pref-1");
  });

  it("keeps join loading state stable on repeated clicks while voice join is running", async () => {
    seedAuthenticatedWorkspace();
    const voiceTokenGate = deferred<void>();
    const fixture = createVoiceFixtureFetch({
      voiceTokenDelay: voiceTokenGate.promise,
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    const joinButton = await screen.findByRole("button", { name: "Join Voice" });
    fireEvent.click(joinButton);
    const joiningButton = await screen.findByRole("button", { name: "Joining..." });
    fireEvent.click(joiningButton);

    expect(fixture.voiceTokenRequestCount()).toBe(1);
    expect(screen.queryByRole("button", { name: "Join Voice" })).not.toBeInTheDocument();
    expect(joiningButton).toBeDisabled();
    expect(rtcMock.join).toHaveBeenCalledTimes(0);

    voiceTokenGate.resolve(undefined);
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(await findVoiceControl("Mute Mic")).toBeInTheDocument();
  });

  it("supports mute/unmute and leave after joining", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    fireEvent.click(await findVoiceControl("Mute Mic"));
    await waitFor(() => expect(rtcMock.toggleMicrophone).toHaveBeenCalledTimes(1));
    expect(await findVoiceControl("Unmute Mic")).toBeInTheDocument();
    expect(fixture.voiceStateBodies()).toContainEqual({
      is_muted: true,
    });

    fireEvent.click(await findVoiceControl("Deafen Audio"));
    await waitFor(() => expect(rtcMock.toggleDeafened).toHaveBeenCalledTimes(1));
    expect(await findVoiceControl("Undeafen Audio")).toBeInTheDocument();
    expect(fixture.voiceStateBodies()).toContainEqual({
      is_deafened: true,
    });

    fireEvent.click(getVoiceControl("Disconnect"));
    await waitFor(() => expect(rtcMock.leave).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole("button", { name: "Join Voice" })).toBeInTheDocument();
  });

  it("supports camera and screen-share toggles when grants allow publishing", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      voicePermissions: [
        "create_message",
        "subscribe_streams",
        "publish_video",
        "publish_screen_share",
      ],
      voiceTokenResponse: {
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone", "camera", "screen_share"],
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    fireEvent.click(await findVoiceControl("Camera On"));
    await waitFor(() => expect(rtcMock.toggleCamera).toHaveBeenCalledTimes(1));
    expect(await findVoiceControl("Camera Off")).toBeInTheDocument();

    fireEvent.click(await findVoiceControl("Share Screen"));
    await waitFor(() => expect(rtcMock.toggleScreenShare).toHaveBeenCalledTimes(1));
    expect(await findVoiceControl("Stop Share")).toBeInTheDocument();
  });

  it("clamps camera/screen controls when token grants do not include those sources", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      voicePermissions: [
        "create_message",
        "subscribe_streams",
        "publish_video",
        "publish_screen_share",
      ],
      voiceTokenResponse: {
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    expect(await findVoiceControl("Camera On")).toBeDisabled();
    const screenShareControl = screen.queryByRole("button", {
      name: /Share Screen|Stop Share/,
    });
    if (screenShareControl) {
      expect(screenShareControl).toBeDisabled();
    }
  });

  it("shows explicit troubleshooting for permission rejection on voice join", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      voiceTokenError: {
        status: 403,
        code: "forbidden",
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    expect(
      await screen.findByText("Voice join rejected by channel permissions or overrides."),
    ).toBeInTheDocument();
  });

  it("shows explicit troubleshooting for expired session while issuing voice token", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      voiceTokenError: {
        status: 401,
        code: "invalid_credentials",
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    expect(
      await screen.findByText(
        "Voice token request expired with your session. Refresh session or login again, then retry Join Voice.",
      ),
    ).toBeInTheDocument();
  });

  it("shows explicit troubleshooting for reconnect and unexpected disconnect", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    rtcMock.setConnectionStatus("reconnecting");
    expect(
      await screen.findByText("Voice reconnecting. Media may recover automatically."),
    ).toBeInTheDocument();

    rtcMock.setConnectionStatus("connected");
    expect(await screen.findByText("Voice reconnected.")).toBeInTheDocument();

    rtcMock.setConnectionStatus("disconnected");
    expect(
      await screen.findByText("Voice connection dropped. Select Join Voice to reconnect."),
    ).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Join Voice" })).toBeInTheDocument();
  });

  it("shows explicit troubleshooting when LiveKit join fails", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setJoinFailure(
      new RtcClientError("join_failed", "Failed to connect to LiveKit: connection refused"),
    );

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText("Voice connection failed. Verify LiveKit signaling reachability and retry."),
    ).toBeInTheDocument();
  });

  it("renders in-call participant roster while voice session is active", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setJoinParticipants([
      { identity: "u.remote.1", subscribedTrackCount: 2 },
      { identity: "u.remote.2", subscribedTrackCount: 1 },
    ]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    expect(await screen.findByLabelText("In-call participants")).toBeInTheDocument();
    expect(screen.getAllByText("u.local (you)")[0]).toBeInTheDocument();
    expect(screen.getAllByText("u.remote.1")[0]).toBeInTheDocument();
    expect(screen.getAllByText("u.remote.2")[0]).toBeInTheDocument();
    expect(screen.queryByLabelText("Voice stream tiles")).not.toBeInTheDocument();
  });

  it("resolves voice participant usernames from cache lookup when identity is livekit-scoped", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch({
      userLookupById: {
        [REMOTE_USER_ID]: "bob",
      },
    });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setLocalParticipantIdentity(`u.${USER_ID}.01ARZ3NDEKTSV4RRFFQ69G5FB2`);
    rtcMock.setJoinParticipants([
      {
        identity: `u.${REMOTE_USER_ID}.01ARZ3NDEKTSV4RRFFQ69G5FB3`,
        subscribedTrackCount: 1,
      },
    ]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    expect((await screen.findAllByText("alice (you)"))[0]).toBeInTheDocument();
    expect((await screen.findAllByText("bob"))[0]).toBeInTheDocument();
    await waitFor(() => {
      const lookupBodies = fixture.userLookupBodies();
      expect(lookupBodies.length).toBeGreaterThan(0);
      expect(JSON.stringify(lookupBodies)).toContain(REMOTE_USER_ID);
    });
  });

  it("shows a speaking ring on participant avatars and clears it after returning to idle", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setJoinParticipants([{ identity: "u.remote.1", subscribedTrackCount: 1 }]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    const remoteAvatar = () =>
      screen.getAllByText("u.remote.1")[0].closest("li")?.querySelector(".voice-tree-avatar") ?? null;

    await waitFor(() => expect(remoteAvatar()).not.toBeNull());
    expect(remoteAvatar()).not.toHaveClass("voice-tree-avatar-speaking");

    rtcMock.setActiveSpeakerIdentities(["u.remote.1"]);
    await waitFor(() => expect(remoteAvatar()).toHaveClass("voice-tree-avatar-speaking"));

    rtcMock.setActiveSpeakerIdentities([]);
    await waitFor(() => expect(remoteAvatar()).not.toHaveClass("voice-tree-avatar-speaking"));
  });

  it("shows a single LIVE badge next to streaming participants in the channel rail", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setJoinParticipants([{ identity: "u.remote.1", subscribedTrackCount: 1 }]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    rtcMock.setVideoTracks(
      [
        {
          trackSid: "L-CAMERA",
          participantIdentity: "u.local",
          source: "camera",
          isLocal: true,
        },
        {
          trackSid: "R-CAMERA",
          participantIdentity: "u.remote.1",
          source: "camera",
          isLocal: false,
        },
        {
          trackSid: "R-SCREEN",
          participantIdentity: "u.remote.1",
          source: "screen_share",
          isLocal: false,
        },
      ],
    );

    await waitFor(() => expect(screen.getAllByText("u.remote.1")[0]).toBeInTheDocument());
    expect(screen.getAllByText("LIVE").length).toBeGreaterThan(0);
    expect(screen.queryByText("Video")).not.toBeInTheDocument();
    expect(screen.queryByText("Share")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Voice stream tiles")).not.toBeInTheDocument();
  });

  it("keeps the voice room active when switching channels in the same workspace", async () => {
    const channels: FixtureChannel[] = [
      { channelId: CHANNEL_ID, name: "bridge", kind: "voice" },
      { channelId: TEXT_CHANNEL_ID, name: "general", kind: "text" },
    ];
    seedAuthenticatedWorkspace(channels);
    const fixture = createVoiceFixtureFetch({ channels });
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));
    expect(await screen.findByLabelText("In-call participants")).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "#general" }));
    await waitFor(() => expect(rtcMock.leave).toHaveBeenCalledTimes(0));
    expect(await screen.findByLabelText("In-call participants")).toBeInTheDocument();
    expect(getVoiceControl("Disconnect")).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "bridge" }));
    expect(await findVoiceControl("Disconnect")).toBeInTheDocument();
  });

  it("logs out cleanly even when rtc leave/destroy teardown rejects", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setLeaveFailure(new Error("leave_failed"));
    rtcMock.setDestroyFailure(new Error("destroy_failed"));

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "Logout" }));
    await waitFor(() => expect(rtcMock.leave).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(rtcMock.destroy).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Welcome Back")).toBeInTheDocument();
    expect(window.sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
    expect(window.localStorage.getItem(WORKSPACE_CACHE_KEY)).toBeNull();
  });

  it("logs out with deterministic RTC teardown and clears local auth state", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "Logout" }));
    await waitFor(() => expect(rtcMock.leave).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(rtcMock.destroy).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Welcome Back")).toBeInTheDocument();
    expect(window.sessionStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
    expect(window.localStorage.getItem(WORKSPACE_CACHE_KEY)).toBeNull();
  });
});
