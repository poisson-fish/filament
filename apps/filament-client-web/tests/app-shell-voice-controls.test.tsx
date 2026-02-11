import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

const rtcMock = vi.hoisted(() => {
  type MockParticipant = { identity: string; subscribedTrackCount: number };
  type MockSnapshot = {
    connectionStatus: "disconnected" | "connecting" | "connected" | "reconnecting" | "error";
    localParticipantIdentity: string | null;
    isMicrophoneEnabled: boolean;
    participants: MockParticipant[];
    activeSpeakerIdentities: string[];
    lastErrorCode: string | null;
    lastErrorMessage: string | null;
  };

  const createDisconnectedSnapshot = (): MockSnapshot => ({
    connectionStatus: "disconnected",
    localParticipantIdentity: null,
    isMicrophoneEnabled: false,
    participants: [],
    activeSpeakerIdentities: [],
    lastErrorCode: null,
    lastErrorMessage: null,
  });

  const listeners = new Set<(snapshot: MockSnapshot) => void>();
  let snapshot = createDisconnectedSnapshot();
  let joinParticipants: MockParticipant[] = [];

  const cloneParticipants = (participants: MockParticipant[]): MockParticipant[] =>
    participants.map((entry) => ({
      identity: entry.identity,
      subscribedTrackCount: entry.subscribedTrackCount,
    }));

  const emit = () => {
    const next = {
      ...snapshot,
      participants: cloneParticipants(snapshot.participants),
    };
    for (const listener of listeners) {
      listener(next);
    }
  };

  const join = vi.fn(async () => {
    snapshot = {
      ...snapshot,
      connectionStatus: "connected",
      localParticipantIdentity: "u.local",
      participants: cloneParticipants(joinParticipants),
      activeSpeakerIdentities: [],
      lastErrorCode: null,
      lastErrorMessage: null,
    };
    emit();
  });

  const leave = vi.fn(async () => {
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

  const destroy = vi.fn(async () => {
    await leave();
    listeners.clear();
  });

  const client = {
    snapshot: () => snapshot,
    subscribe(listener: (nextSnapshot: MockSnapshot) => void) {
      listeners.add(listener);
      listener({
        ...snapshot,
        participants: cloneParticipants(snapshot.participants),
      });
      return () => {
        listeners.delete(listener);
      };
    },
    join,
    leave,
    setMicrophoneEnabled,
    toggleMicrophone,
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

  const reset = () => {
    listeners.clear();
    snapshot = createDisconnectedSnapshot();
    joinParticipants = [];
    createRtcClient.mockClear();
    join.mockClear();
    leave.mockClear();
    setMicrophoneEnabled.mockClear();
    toggleMicrophone.mockClear();
    destroy.mockClear();
  };

  return {
    createRtcClient,
    join,
    leave,
    setMicrophoneEnabled,
    toggleMicrophone,
    destroy,
    setJoinParticipants,
    setActiveSpeakerIdentities,
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

function seedAuthenticatedWorkspace(): void {
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
        channels: [{ channelId: CHANNEL_ID, name: "bridge", kind: "voice" }],
      },
    ]),
  );
}

function createVoiceFixtureFetch() {
  let voiceTokenBody: unknown = null;

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
        channels: [{ channel_id: CHANNEL_ID, name: "bridge", kind: "voice" }],
      });
    }

    if (method === "GET" && url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/permissions/self`)) {
      return jsonResponse({
        role: "member",
        permissions: ["create_message", "subscribe_streams"],
      });
    }

    if (method === "GET" && url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)) {
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

    if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/voice/token`)) {
      voiceTokenBody = init?.body ? JSON.parse(init.body as string) : null;
      return jsonResponse({
        token: "T".repeat(96),
        livekit_url: "wss://livekit.example.com",
        room: "filament.voice.room",
        identity: "u.identity.123",
        can_publish: true,
        can_subscribe: true,
        publish_sources: ["microphone"],
        expires_in_secs: 300,
      });
    }

    if (method === "POST" && url.endsWith("/auth/logout")) {
      return new Response(null, { status: 204 });
    }

    return jsonResponse({ error: "not_found" }, 404);
  });

  return {
    fetchMock,
    voiceTokenBody: () => voiceTokenBody,
  };
}

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

    expect(await screen.findByRole("button", { name: "Mute Mic" })).toBeInTheDocument();
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

    fireEvent.click(await screen.findByRole("button", { name: "Mute Mic" }));
    await waitFor(() => expect(rtcMock.toggleMicrophone).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole("button", { name: "Unmute Mic" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    await waitFor(() => expect(rtcMock.leave).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole("button", { name: "Join Voice" })).toBeInTheDocument();
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

    expect(await screen.findByText("In call (3)")).toBeInTheDocument();
    expect(screen.getByText("u.local (you)")).toBeInTheDocument();
    expect(screen.getByText("u.remote.1")).toBeInTheDocument();
    expect(screen.getByText("u.remote.2")).toBeInTheDocument();
  });

  it("highlights active speakers and clears highlight after returning to idle", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createVoiceFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    rtcMock.setJoinParticipants([{ identity: "u.remote.1", subscribedTrackCount: 1 }]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Join Voice" }));
    await waitFor(() => expect(rtcMock.join).toHaveBeenCalledTimes(1));

    expect(await screen.findByText("u.remote.1")).not.toHaveClass("voice-roster-name-speaking");

    rtcMock.setActiveSpeakerIdentities(["u.remote.1"]);
    await waitFor(() =>
      expect(screen.getByText("u.remote.1")).toHaveClass("voice-roster-name-speaking"),
    );

    rtcMock.setActiveSpeakerIdentities([]);
    await waitFor(() =>
      expect(screen.getByText("u.remote.1")).not.toHaveClass("voice-roster-name-speaking"),
    );
  });
});
