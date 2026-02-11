import { ConnectionState, RoomEvent } from "livekit-client";
import type { LivekitToken, LivekitUrl } from "../src/domain/chat";
import {
  RTC_DEFAULT_MAX_PARTICIPANTS,
  RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT,
  createRtcClient,
  rtcTokenFromInput,
  rtcUrlFromInput,
  type RtcSnapshot,
  RtcClientError,
} from "../src/lib/rtc";

type RoomListener = (...args: unknown[]) => void;

interface MockRemoteParticipant {
  identity: string;
  trackPublications: Map<string, { trackSid: string }>;
}

function buildRemoteParticipant(identity: string, trackSids: string[] = []): MockRemoteParticipant {
  const trackPublications = new Map<string, { trackSid: string }>();
  for (const sid of trackSids) {
    trackPublications.set(sid, { trackSid: sid });
  }
  return {
    identity,
    trackPublications,
  };
}

class MockRoom {
  state: ConnectionState = ConnectionState.Disconnected;
  readonly localParticipant = {
    identity: "local-user",
    setMicrophoneEnabled: vi.fn(async (_enabled: boolean) => {}),
  };
  readonly remoteParticipants = new Map<string, MockRemoteParticipant>();
  readonly connectCalls: Array<{ url: string; token: string }> = [];
  disconnectCalls = 0;
  connectFailure: Error | null = null;
  private readonly listeners = new Map<RoomEvent, Set<RoomListener>>();

  on(event: RoomEvent, listener: RoomListener): void {
    const current = this.listeners.get(event) ?? new Set<RoomListener>();
    current.add(listener);
    this.listeners.set(event, current);
  }

  off(event: RoomEvent, listener: RoomListener): void {
    const current = this.listeners.get(event);
    if (!current) {
      return;
    }
    current.delete(listener);
    if (current.size === 0) {
      this.listeners.delete(event);
    }
  }

  emit(event: RoomEvent, ...args: unknown[]): void {
    const current = this.listeners.get(event);
    if (!current) {
      return;
    }
    for (const listener of current) {
      listener(...args);
    }
  }

  async connect(url: string, token: string): Promise<void> {
    this.connectCalls.push({ url, token });
    if (this.connectFailure) {
      throw this.connectFailure;
    }
    this.state = ConnectionState.Connected;
    this.emit(RoomEvent.ConnectionStateChanged, ConnectionState.Connected);
    this.emit(RoomEvent.Connected);
  }

  async disconnect(): Promise<void> {
    this.disconnectCalls += 1;
    this.state = ConnectionState.Disconnected;
    this.emit(RoomEvent.ConnectionStateChanged, ConnectionState.Disconnected);
    this.emit(RoomEvent.Disconnected);
  }

  listenerCount(): number {
    let total = 0;
    for (const listeners of this.listeners.values()) {
      total += listeners.size;
    }
    return total;
  }
}

describe("rtc URL/token validation", () => {
  it("accepts websocket URLs and rejects invalid schemes", () => {
    expect(rtcUrlFromInput("ws://127.0.0.1:7880")).toBe("ws://127.0.0.1:7880/");
    expect(rtcUrlFromInput("wss://livekit.example.com")).toBe("wss://livekit.example.com/");

    expect(() => rtcUrlFromInput("https://livekit.example.com")).toThrow(RtcClientError);
    expect(() => rtcUrlFromInput("wss://user:pass@livekit.example.com")).toThrow(RtcClientError);
    expect(() => rtcUrlFromInput("")).toThrow(RtcClientError);
  });

  it("accepts bounded printable token values", () => {
    expect(rtcTokenFromInput("A".repeat(96))).toBe("A".repeat(96));
    expect(() => rtcTokenFromInput("")).toThrow(RtcClientError);
    expect(() => rtcTokenFromInput("contains whitespace")).toThrow(RtcClientError);
    expect(() => rtcTokenFromInput("A".repeat(8_193))).toThrow(RtcClientError);
  });
});

describe("rtc client lifecycle", () => {
  const validUrl = rtcUrlFromInput("wss://livekit.example.com");
  const validToken = rtcTokenFromInput("A".repeat(96));

  it("joins, seeds participants, and leaves cleanly", async () => {
    const room = new MockRoom();
    room.remoteParticipants.set("alice", buildRemoteParticipant("alice", ["TRK1", "TRK2"]));

    const client = createRtcClient({
      roomFactory: () => room,
    });

    const snapshots: RtcSnapshot[] = [];
    const unsubscribe = client.subscribe((snapshot) => snapshots.push(snapshot));

    await client.join({
      livekitUrl: validUrl,
      token: validToken,
    });

    expect(room.connectCalls).toHaveLength(1);
    expect(snapshots.some((snapshot) => snapshot.connectionStatus === "connecting")).toBe(true);
    expect(client.snapshot().connectionStatus).toBe("connected");
    expect(client.snapshot().participants).toEqual([
      {
        identity: "alice",
        subscribedTrackCount: 2,
      },
    ]);

    await client.leave();

    expect(room.disconnectCalls).toBe(1);
    expect(client.snapshot().connectionStatus).toBe("disconnected");
    expect(client.snapshot().participants).toEqual([]);
    unsubscribe();
  });

  it("tears down the previous room on rejoin", async () => {
    const roomOne = new MockRoom();
    const roomTwo = new MockRoom();
    const rooms = [roomOne, roomTwo];

    const client = createRtcClient({
      roomFactory: () => {
        const next = rooms.shift();
        if (!next) {
          throw new Error("missing room");
        }
        return next;
      },
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    await client.join({ livekitUrl: validUrl, token: validToken });

    expect(roomOne.disconnectCalls).toBe(1);
    expect(roomTwo.connectCalls).toHaveLength(1);
  });

  it("updates local microphone state via set/toggle", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    await client.setMicrophoneEnabled(true);
    const next = await client.toggleMicrophone();

    expect(next).toBe(false);
    expect(room.localParticipant.setMicrophoneEnabled).toHaveBeenNthCalledWith(1, true);
    expect(room.localParticipant.setMicrophoneEnabled).toHaveBeenNthCalledWith(2, false);
    expect(client.snapshot().isMicrophoneEnabled).toBe(false);
  });

  it("fails closed for invalid URL inputs before connection attempts", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await expect(
      client.join({
        livekitUrl: "https://invalid.example.com" as LivekitUrl,
        token: validToken,
      }),
    ).rejects.toMatchObject({
      code: "invalid_livekit_url",
    });
    expect(room.connectCalls).toHaveLength(0);
  });

  it("surfaces join errors and clears tracked state", async () => {
    const room = new MockRoom();
    room.connectFailure = new Error("connect_failed");
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await expect(
      client.join({
        livekitUrl: validUrl,
        token: validToken,
      }),
    ).rejects.toMatchObject({
      code: "join_failed",
    });

    expect(client.snapshot().connectionStatus).toBe("error");
    expect(client.snapshot().lastErrorCode).toBe("join_failed");
    expect(client.snapshot().participants).toEqual([]);
    expect(room.disconnectCalls).toBe(1);
  });

  it("bounds participant and track state growth", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
      maxParticipants: 2,
      maxTracksPerParticipant: 1,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });

    const alpha = buildRemoteParticipant("alpha");
    const beta = buildRemoteParticipant("beta");
    const gamma = buildRemoteParticipant("gamma");
    room.emit(RoomEvent.ParticipantConnected, alpha);
    room.emit(RoomEvent.ParticipantConnected, beta);
    room.emit(RoomEvent.ParticipantConnected, gamma);

    room.emit(RoomEvent.TrackSubscribed, null, { trackSid: "T1" }, alpha);
    room.emit(RoomEvent.TrackSubscribed, null, { trackSid: "T2" }, alpha);

    const snapshot = client.snapshot();
    expect(snapshot.participants).toEqual([
      { identity: "alpha", subscribedTrackCount: 1 },
      { identity: "beta", subscribedTrackCount: 0 },
    ]);
  });

  it("removes all listeners on destroy", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    expect(room.listenerCount()).toBeGreaterThan(0);

    await client.destroy();

    expect(room.disconnectCalls).toBe(1);
    expect(room.listenerCount()).toBe(0);
  });
});

describe("rtc defaults", () => {
  it("retains phase baseline bounds", () => {
    expect(RTC_DEFAULT_MAX_PARTICIPANTS).toBe(256);
    expect(RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT).toBe(32);
  });

  it("validates custom bounds", () => {
    expect(() =>
      createRtcClient({
        maxParticipants: 0,
      }),
    ).toThrow(RtcClientError);
    expect(() =>
      createRtcClient({
        maxTracksPerParticipant: 0,
      }),
    ).toThrow(RtcClientError);
    expect(() =>
      createRtcClient({
        maxParticipants: Number.NaN,
      }),
    ).toThrow(RtcClientError);
    expect(() =>
      createRtcClient({
        maxTracksPerParticipant: Number.NaN,
      }),
    ).toThrow(RtcClientError);
  });

  it("rejects malformed token in join path", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await expect(
      client.join({
        livekitUrl: rtcUrlFromInput("wss://livekit.example.com"),
        token: "token with spaces" as LivekitToken,
      }),
    ).rejects.toMatchObject({
      code: "invalid_livekit_token",
    });
    expect(room.connectCalls).toHaveLength(0);
  });
});
