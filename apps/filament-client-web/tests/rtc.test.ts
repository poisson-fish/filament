import { ConnectionState, RoomEvent } from "livekit-client";
import type { LivekitToken, LivekitUrl } from "../src/domain/chat";
import {
  RTC_DEFAULT_ACTIVE_SPEAKER_DEBOUNCE_MS,
  RTC_DEFAULT_ACTIVE_SPEAKER_HYSTERESIS_MS,
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
  trackPublications: Map<string, MockTrackPublication>;
}

interface MockTrackPublication {
  trackSid: string;
  source?: "camera" | "screen_share" | "microphone";
}

interface MockAudioTrack {
  kind: "audio";
  attach: ReturnType<typeof vi.fn>;
  detach: ReturnType<typeof vi.fn>;
}

function buildTrackPublication(
  trackSid: string,
  source?: "camera" | "screen_share" | "microphone",
): MockTrackPublication {
  return source ? { trackSid, source } : { trackSid };
}

function buildRemoteParticipant(
  identity: string,
  publications: Array<string | MockTrackPublication> = [],
): MockRemoteParticipant {
  const trackPublications = new Map<string, MockTrackPublication>();
  for (const publication of publications) {
    const next = typeof publication === "string" ? buildTrackPublication(publication) : publication;
    trackPublications.set(next.trackSid, next);
  }
  return {
    identity,
    trackPublications,
  };
}

function buildAudioTrack(): MockAudioTrack {
  return {
    kind: "audio",
    attach: vi.fn((element?: HTMLMediaElement) => element ?? document.createElement("audio")),
    detach: vi.fn(),
  };
}

class MockRoom {
  state: ConnectionState = ConnectionState.Disconnected;
  private localCameraTrackSid: string | null = null;
  private localScreenTrackSid: string | null = null;
  private localTrackCounter = 0;
  readonly localParticipant = {
    identity: "local-user",
    isCameraEnabled: false,
    isScreenShareEnabled: false,
    videoTrackPublications: new Map<string, MockTrackPublication>(),
    setMicrophoneEnabled: vi.fn(async (_enabled: boolean) => {}),
    setCameraEnabled: vi.fn(async (enabled: boolean) => {
      this.localParticipant.isCameraEnabled = enabled;
      this.setLocalVideoTrack("camera", enabled);
    }),
    setScreenShareEnabled: vi.fn(async (enabled: boolean) => {
      this.localParticipant.isScreenShareEnabled = enabled;
      this.setLocalVideoTrack("screen_share", enabled);
    }),
  };
  readonly remoteParticipants = new Map<string, MockRemoteParticipant>();
  readonly connectCalls: Array<{ url: string; token: string }> = [];
  readonly switchDeviceCalls: Array<{
    kind: MediaDeviceKind;
    deviceId: string;
    exact: boolean | undefined;
  }> = [];
  disconnectCalls = 0;
  connectFailure: Error | null = null;
  switchDeviceFailure: Error | null = null;
  switchDeviceResult = true;
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

  async switchActiveDevice(
    kind: MediaDeviceKind,
    deviceId: string,
    exact?: boolean,
  ): Promise<boolean> {
    this.switchDeviceCalls.push({ kind, deviceId, exact });
    if (this.switchDeviceFailure) {
      throw this.switchDeviceFailure;
    }
    return this.switchDeviceResult;
  }

  listenerCount(): number {
    let total = 0;
    for (const listeners of this.listeners.values()) {
      total += listeners.size;
    }
    return total;
  }

  private nextLocalTrackSid(prefix: string): string {
    this.localTrackCounter += 1;
    return `${prefix}-${this.localTrackCounter}`;
  }

  private setLocalVideoTrack(source: "camera" | "screen_share", enabled: boolean): void {
    const sid =
      source === "camera" ? this.localCameraTrackSid : this.localScreenTrackSid;
    if (!enabled) {
      if (!sid) {
        return;
      }
      this.localParticipant.videoTrackPublications.delete(sid);
      if (source === "camera") {
        this.localCameraTrackSid = null;
      } else {
        this.localScreenTrackSid = null;
      }
      this.emit(RoomEvent.LocalTrackUnpublished, buildTrackPublication(sid, source), this.localParticipant);
      return;
    }

    const nextSid = sid ?? this.nextLocalTrackSid(source === "camera" ? "LCAM" : "LSCR");
    this.localParticipant.videoTrackPublications.set(nextSid, buildTrackPublication(nextSid, source));
    if (source === "camera") {
      this.localCameraTrackSid = nextSid;
    } else {
      this.localScreenTrackSid = nextSid;
    }
    this.emit(RoomEvent.LocalTrackPublished, buildTrackPublication(nextSid, source), this.localParticipant);
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

  it("updates camera and screen share state via set/toggle", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    await client.setCameraEnabled(true);
    const cameraNext = await client.toggleCamera();
    await client.setScreenShareEnabled(true);
    const screenNext = await client.toggleScreenShare();

    expect(cameraNext).toBe(false);
    expect(screenNext).toBe(false);
    expect(room.localParticipant.setCameraEnabled).toHaveBeenNthCalledWith(1, true);
    expect(room.localParticipant.setCameraEnabled).toHaveBeenNthCalledWith(2, false);
    expect(room.localParticipant.setScreenShareEnabled).toHaveBeenNthCalledWith(1, true);
    expect(room.localParticipant.setScreenShareEnabled).toHaveBeenNthCalledWith(2, false);
    expect(client.snapshot().isCameraEnabled).toBe(false);
    expect(client.snapshot().isScreenShareEnabled).toBe(false);
  });

  it("stages preferred audio devices and applies them during join", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.setAudioInputDevice("mic-01");
    await client.setAudioOutputDevice("spk-09");
    await client.join({ livekitUrl: validUrl, token: validToken });

    expect(room.switchDeviceCalls).toEqual([
      { kind: "audioinput", deviceId: "mic-01", exact: true },
      { kind: "audiooutput", deviceId: "spk-09", exact: true },
    ]);
  });

  it("switches preferred audio devices while connected", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    await client.setAudioInputDevice("mic-11");
    await client.setAudioOutputDevice("spk-11");

    expect(room.switchDeviceCalls).toEqual([
      { kind: "audioinput", deviceId: "mic-11", exact: true },
      { kind: "audiooutput", deviceId: "spk-11", exact: true },
    ]);
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

  it("rejects malformed audio device IDs", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await expect(client.setAudioInputDevice("")).rejects.toMatchObject({
      code: "invalid_audio_device_id",
    });
    await expect(client.setAudioOutputDevice("\n")).rejects.toMatchObject({
      code: "invalid_audio_device_id",
    });
    expect(room.switchDeviceCalls).toHaveLength(0);
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

  it("keeps voice connected when audio-device apply fails during join", async () => {
    const room = new MockRoom();
    room.switchDeviceFailure = new Error("not_found");
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.setAudioInputDevice("missing-mic");
    await client.join({
      livekitUrl: validUrl,
      token: validToken,
    });

    expect(client.snapshot().connectionStatus).toBe("connected");
    expect(client.snapshot().lastErrorCode).toBe("audio_device_switch_failed");
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

  it("reflects reconnect transitions and clears tracked state on disconnect", async () => {
    const room = new MockRoom();
    room.remoteParticipants.set("alice", buildRemoteParticipant("alice", ["TRK1"]));
    const client = createRtcClient({
      roomFactory: () => room,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    expect(client.snapshot().connectionStatus).toBe("connected");
    expect(client.snapshot().participants).toEqual([
      {
        identity: "alice",
        subscribedTrackCount: 1,
      },
    ]);

    room.emit(RoomEvent.ConnectionStateChanged, ConnectionState.Reconnecting);
    expect(client.snapshot().connectionStatus).toBe("reconnecting");
    expect(client.snapshot().participants).toEqual([
      {
        identity: "alice",
        subscribedTrackCount: 1,
      },
    ]);

    room.emit(RoomEvent.ConnectionStateChanged, ConnectionState.Disconnected);
    expect(client.snapshot().connectionStatus).toBe("disconnected");
    expect(client.snapshot().participants).toEqual([]);
  });

  it("tracks local and remote camera/screen streams with bounded tile identities", async () => {
    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
      maxParticipants: 2,
    });

    await client.join({ livekitUrl: validUrl, token: validToken });
    const alpha = buildRemoteParticipant("alpha");
    room.emit(RoomEvent.ParticipantConnected, alpha);

    room.emit(
      RoomEvent.TrackSubscribed,
      { trackSid: "RV1", source: "camera" },
      buildTrackPublication("RV1", "camera"),
      alpha,
    );
    room.emit(
      RoomEvent.TrackSubscribed,
      { trackSid: "RV2", source: "screen_share" },
      buildTrackPublication("RV2", "screen_share"),
      alpha,
    );
    room.emit(
      RoomEvent.TrackSubscribed,
      { trackSid: "RA1", source: "microphone" },
      buildTrackPublication("RA1", "microphone"),
      alpha,
    );
    await client.setCameraEnabled(true);
    await client.setScreenShareEnabled(true);

    expect(
      client
        .snapshot()
        .videoTracks.map((track) => `${track.isLocal ? "local" : "remote"}:${track.participantIdentity}:${track.source}`),
    ).toEqual([
      "local:local-user:camera",
      "local:local-user:screen_share",
      "remote:alpha:camera",
      "remote:alpha:screen_share",
    ]);

    room.emit(RoomEvent.TrackUnpublished, buildTrackPublication("RV1", "camera"), alpha);
    expect(
      client
        .snapshot()
        .videoTracks.map((track) => `${track.isLocal ? "local" : "remote"}:${track.participantIdentity}:${track.source}`),
    ).toEqual([
      "local:local-user:camera",
      "local:local-user:screen_share",
      "remote:alpha:screen_share",
    ]);
  });

  it("attaches remote microphone tracks for playback and detaches on unsubscribe", async () => {
    document.body.innerHTML = "";

    const room = new MockRoom();
    const client = createRtcClient({
      roomFactory: () => room,
    });
    const alpha = buildRemoteParticipant("alpha");

    await client.join({ livekitUrl: validUrl, token: validToken });
    room.emit(RoomEvent.ParticipantConnected, alpha);

    const publication = buildTrackPublication("AUD1", "microphone");
    const audioTrack = buildAudioTrack();
    room.emit(RoomEvent.TrackSubscribed, audioTrack, publication, alpha);

    expect(audioTrack.attach).toHaveBeenCalledTimes(1);
    expect(document.querySelector('audio[data-track-sid="AUD1"]')).not.toBeNull();

    room.emit(RoomEvent.TrackUnsubscribed, audioTrack, publication, alpha);

    expect(audioTrack.detach).toHaveBeenCalledTimes(1);
    expect(document.querySelector('audio[data-track-sid="AUD1"]')).toBeNull();
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

  it("transitions active speaker state with debounce and hysteresis", async () => {
    vi.useFakeTimers();
    try {
      const room = new MockRoom();
      const alpha = buildRemoteParticipant("alpha");
      room.remoteParticipants.set("alpha", alpha);
      const client = createRtcClient({
        roomFactory: () => room,
        activeSpeakerDebounceMs: 100,
        activeSpeakerHysteresisMs: 300,
      });

      await client.join({ livekitUrl: validUrl, token: validToken });
      expect(client.snapshot().activeSpeakerIdentities).toEqual([]);

      room.emit(RoomEvent.ActiveSpeakersChanged, [alpha]);
      await vi.advanceTimersByTimeAsync(99);
      expect(client.snapshot().activeSpeakerIdentities).toEqual([]);

      await vi.advanceTimersByTimeAsync(1);
      expect(client.snapshot().activeSpeakerIdentities).toEqual(["alpha"]);

      room.emit(RoomEvent.ActiveSpeakersChanged, []);
      await vi.advanceTimersByTimeAsync(299);
      expect(client.snapshot().activeSpeakerIdentities).toEqual(["alpha"]);

      await vi.advanceTimersByTimeAsync(1);
      expect(client.snapshot().activeSpeakerIdentities).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("reconciles active speaker state when speaker arrives before participant registration", async () => {
    vi.useFakeTimers();
    try {
      const room = new MockRoom();
      const alpha = buildRemoteParticipant("alpha");
      const client = createRtcClient({
        roomFactory: () => room,
        activeSpeakerDebounceMs: 100,
        activeSpeakerHysteresisMs: 300,
      });

      await client.join({ livekitUrl: validUrl, token: validToken });

      room.emit(RoomEvent.ActiveSpeakersChanged, [alpha]);
      await vi.advanceTimersByTimeAsync(100);
      expect(client.snapshot().activeSpeakerIdentities).toEqual([]);

      room.emit(RoomEvent.ParticipantConnected, alpha);
      await vi.advanceTimersByTimeAsync(99);
      expect(client.snapshot().activeSpeakerIdentities).toEqual([]);

      await vi.advanceTimersByTimeAsync(1);
      expect(client.snapshot().activeSpeakerIdentities).toEqual(["alpha"]);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("rtc defaults", () => {
  it("retains phase baseline bounds", () => {
    expect(RTC_DEFAULT_MAX_PARTICIPANTS).toBe(256);
    expect(RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT).toBe(32);
    expect(RTC_DEFAULT_ACTIVE_SPEAKER_DEBOUNCE_MS).toBe(180);
    expect(RTC_DEFAULT_ACTIVE_SPEAKER_HYSTERESIS_MS).toBe(900);
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
    expect(() =>
      createRtcClient({
        activeSpeakerDebounceMs: -1,
      }),
    ).toThrow(RtcClientError);
    expect(() =>
      createRtcClient({
        activeSpeakerHysteresisMs: -1,
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
