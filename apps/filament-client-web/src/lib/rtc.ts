import {
  ConnectionState,
  Room,
  RoomEvent,
  type RoomConnectOptions,
  type RoomOptions,
} from "livekit-client";
import type { LivekitToken, LivekitUrl } from "../domain/chat";

const MAX_LIVEKIT_URL_CHARS = 2_048;
const MAX_LIVEKIT_TOKEN_CHARS = 8_192;
const MAX_IDENTITY_CHARS = 512;
const MAX_TRACK_SID_CHARS = 128;
const MAX_ERROR_MESSAGE_CHARS = 256;
const MAX_SUBSCRIBERS = 32;
const LIVEKIT_TOKEN_PATTERN = /^[\x21-\x7e]+$/;

export const RTC_DEFAULT_MAX_PARTICIPANTS = 256;
export const RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT = 32;

export type RtcConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type RtcErrorCode =
  | "invalid_livekit_url"
  | "invalid_livekit_token"
  | "join_failed"
  | "not_connected"
  | "microphone_toggle_failed"
  | "listener_limit_exceeded";

export interface RtcParticipantSnapshot {
  identity: string;
  subscribedTrackCount: number;
}

export interface RtcSnapshot {
  connectionStatus: RtcConnectionStatus;
  localParticipantIdentity: string | null;
  isMicrophoneEnabled: boolean;
  participants: RtcParticipantSnapshot[];
  lastErrorCode: RtcErrorCode | null;
  lastErrorMessage: string | null;
}

export interface RtcJoinRequest {
  livekitUrl: LivekitUrl;
  token: LivekitToken;
}

export interface RtcClient {
  snapshot(): RtcSnapshot;
  subscribe(listener: (snapshot: RtcSnapshot) => void): () => void;
  join(request: RtcJoinRequest): Promise<void>;
  leave(): Promise<void>;
  setMicrophoneEnabled(enabled: boolean): Promise<void>;
  toggleMicrophone(): Promise<boolean>;
  destroy(): Promise<void>;
}

export class RtcClientError extends Error {
  readonly code: RtcErrorCode;

  constructor(code: RtcErrorCode, message: string) {
    super(message);
    this.name = "RtcClientError";
    this.code = code;
  }
}

interface TrackedParticipant {
  identity: string;
  trackSids: Set<string>;
}

interface TrackPublicationLike {
  trackSid?: unknown;
}

interface RemoteParticipantLike {
  identity?: unknown;
  trackPublications: Map<string, TrackPublicationLike>;
}

interface LocalParticipantLike {
  identity?: unknown;
  setMicrophoneEnabled(enabled: boolean): Promise<unknown>;
}

type RoomListener = (...args: unknown[]) => void;

interface RtcRoomLike {
  state: ConnectionState;
  localParticipant: LocalParticipantLike;
  remoteParticipants: Map<string, RemoteParticipantLike>;
  on(event: RoomEvent, listener: RoomListener): unknown;
  off(event: RoomEvent, listener: RoomListener): unknown;
  connect(url: string, token: string, options?: RoomConnectOptions): Promise<void>;
  disconnect(stopTracks?: boolean): Promise<void>;
}

interface RegisteredRoomListener {
  event: RoomEvent;
  listener: RoomListener;
}

interface CreateRtcClientOptions {
  roomOptions?: RoomOptions;
  connectOptions?: RoomConnectOptions;
  roomFactory?: (options?: RoomOptions) => RtcRoomLike;
  maxParticipants?: number;
  maxTracksPerParticipant?: number;
}

function sanitizeErrorMessage(error: unknown): string {
  const fallback = "Unknown RTC error.";
  if (!error || typeof error !== "object") {
    return fallback;
  }
  const message = (error as { message?: unknown }).message;
  if (typeof message !== "string" || message.length === 0) {
    return fallback;
  }
  return message.slice(0, MAX_ERROR_MESSAGE_CHARS);
}

function readIdentity(participant: { identity?: unknown } | null | undefined): string | null {
  if (!participant || typeof participant.identity !== "string") {
    return null;
  }
  const value = participant.identity.trim();
  if (value.length < 1 || value.length > MAX_IDENTITY_CHARS) {
    return null;
  }
  return value;
}

function readTrackSid(publication: unknown): string | null {
  if (!publication || typeof publication !== "object") {
    return null;
  }
  const sid = (publication as { trackSid?: unknown }).trackSid;
  if (typeof sid !== "string") {
    return null;
  }
  if (sid.length < 1 || sid.length > MAX_TRACK_SID_CHARS) {
    return null;
  }
  return sid;
}

function collectTrackSids(
  trackPublications: Map<string, TrackPublicationLike>,
  maxTracks: number,
): Set<string> {
  const trackSids = new Set<string>();
  for (const publication of trackPublications.values()) {
    if (trackSids.size >= maxTracks) {
      break;
    }
    const sid = readTrackSid(publication);
    if (!sid) {
      continue;
    }
    trackSids.add(sid);
  }
  return trackSids;
}

function toRtcConnectionStatus(state: ConnectionState): RtcConnectionStatus {
  if (state === ConnectionState.Connected) {
    return "connected";
  }
  if (state === ConnectionState.Connecting) {
    return "connecting";
  }
  if (state === ConnectionState.Reconnecting || state === ConnectionState.SignalReconnecting) {
    return "reconnecting";
  }
  return "disconnected";
}

function isConnectionState(value: unknown): value is ConnectionState {
  return (
    value === ConnectionState.Connected ||
    value === ConnectionState.Connecting ||
    value === ConnectionState.Disconnected ||
    value === ConnectionState.Reconnecting ||
    value === ConnectionState.SignalReconnecting
  );
}

function requirePositiveInteger(
  value: number | undefined,
  fallback: number,
  label: string,
): number {
  if (value === undefined) {
    return fallback;
  }
  if (!Number.isInteger(value) || value < 1 || value > 4_096) {
    throw new RtcClientError("join_failed", `${label} must be an integer between 1 and 4096.`);
  }
  return value;
}

export function rtcUrlFromInput(input: string): LivekitUrl {
  const value = input.trim();
  if (value.length < 1 || value.length > MAX_LIVEKIT_URL_CHARS) {
    throw new RtcClientError("invalid_livekit_url", "LiveKit URL is empty or too long.");
  }

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new RtcClientError("invalid_livekit_url", "LiveKit URL must be absolute.");
  }

  if (parsed.protocol !== "ws:" && parsed.protocol !== "wss:") {
    throw new RtcClientError("invalid_livekit_url", "LiveKit URL must use ws:// or wss://.");
  }
  if (parsed.username.length > 0 || parsed.password.length > 0) {
    throw new RtcClientError("invalid_livekit_url", "LiveKit URL must not include credentials.");
  }
  if (parsed.hash.length > 0) {
    throw new RtcClientError("invalid_livekit_url", "LiveKit URL must not include URL fragments.");
  }

  return parsed.toString() as LivekitUrl;
}

export function rtcTokenFromInput(input: string): LivekitToken {
  if (input.length < 1 || input.length > MAX_LIVEKIT_TOKEN_CHARS) {
    throw new RtcClientError("invalid_livekit_token", "LiveKit token is empty or too long.");
  }
  if (!LIVEKIT_TOKEN_PATTERN.test(input)) {
    throw new RtcClientError(
      "invalid_livekit_token",
      "LiveKit token contains invalid characters.",
    );
  }
  return input as LivekitToken;
}

class RtcClientImpl implements RtcClient {
  private readonly roomOptions?: RoomOptions;
  private readonly connectOptions?: RoomConnectOptions;
  private readonly roomFactory: (options?: RoomOptions) => RtcRoomLike;
  private readonly maxParticipants: number;
  private readonly maxTracksPerParticipant: number;
  private readonly subscribers = new Set<(snapshot: RtcSnapshot) => void>();
  private readonly participants = new Map<string, TrackedParticipant>();
  private activeRoom: RtcRoomLike | null = null;
  private roomListeners: RegisteredRoomListener[] = [];
  private queuedOperation: Promise<void> = Promise.resolve();
  private connectionStatus: RtcConnectionStatus = "disconnected";
  private localParticipantIdentity: string | null = null;
  private isMicrophoneEnabled = false;
  private lastError: { code: RtcErrorCode; message: string } | null = null;

  constructor(options: CreateRtcClientOptions) {
    this.roomOptions = options.roomOptions;
    this.connectOptions = options.connectOptions;
    this.roomFactory =
      options.roomFactory ??
      ((roomOptions) => new Room(roomOptions) as unknown as RtcRoomLike);
    this.maxParticipants = requirePositiveInteger(
      options.maxParticipants,
      RTC_DEFAULT_MAX_PARTICIPANTS,
      "maxParticipants",
    );
    this.maxTracksPerParticipant = requirePositiveInteger(
      options.maxTracksPerParticipant,
      RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT,
      "maxTracksPerParticipant",
    );
  }

  snapshot(): RtcSnapshot {
    const participants = Array.from(this.participants.values())
      .map((participant) => ({
        identity: participant.identity,
        subscribedTrackCount: participant.trackSids.size,
      }))
      .sort((left, right) => left.identity.localeCompare(right.identity));

    return {
      connectionStatus: this.connectionStatus,
      localParticipantIdentity: this.localParticipantIdentity,
      isMicrophoneEnabled: this.isMicrophoneEnabled,
      participants,
      lastErrorCode: this.lastError?.code ?? null,
      lastErrorMessage: this.lastError?.message ?? null,
    };
  }

  subscribe(listener: (snapshot: RtcSnapshot) => void): () => void {
    if (this.subscribers.size >= MAX_SUBSCRIBERS) {
      throw new RtcClientError(
        "listener_limit_exceeded",
        "Too many RTC subscribers registered.",
      );
    }
    this.subscribers.add(listener);
    listener(this.snapshot());
    return () => {
      this.subscribers.delete(listener);
    };
  }

  join(request: RtcJoinRequest): Promise<void> {
    return this.runSerialized(async () => {
      const livekitUrl = rtcUrlFromInput(request.livekitUrl);
      const token = rtcTokenFromInput(request.token);

      await this.disconnectActiveRoom();
      this.clearTrackedState();
      this.lastError = null;
      this.connectionStatus = "connecting";
      this.emitSnapshot();

      const room = this.roomFactory(this.roomOptions);
      this.activeRoom = room;
      this.bindRoomListeners(room);

      try {
        await room.connect(livekitUrl, token, this.connectOptions);
      } catch (error) {
        const message = sanitizeErrorMessage(error);
        this.lastError = {
          code: "join_failed",
          message: `Failed to connect to LiveKit: ${message}`,
        };
        await this.disconnectRoom(room);
        if (this.activeRoom === room) {
          this.activeRoom = null;
        }
        this.clearTrackedState();
        this.connectionStatus = "error";
        this.emitSnapshot();
        throw new RtcClientError("join_failed", this.lastError.message);
      }

      this.localParticipantIdentity = readIdentity(room.localParticipant);
      this.seedParticipants(room);
      this.lastError = null;
      this.connectionStatus = toRtcConnectionStatus(room.state);
      this.emitSnapshot();
    });
  }

  leave(): Promise<void> {
    return this.runSerialized(async () => {
      await this.disconnectActiveRoom();
      this.clearTrackedState();
      this.lastError = null;
      this.connectionStatus = "disconnected";
      this.emitSnapshot();
    });
  }

  setMicrophoneEnabled(enabled: boolean): Promise<void> {
    return this.runSerialized(async () => {
      await this.setMicrophoneEnabledInternal(enabled);
    });
  }

  toggleMicrophone(): Promise<boolean> {
    return this.runSerialized(async () => {
      const next = !this.isMicrophoneEnabled;
      await this.setMicrophoneEnabledInternal(next);
      return next;
    });
  }

  destroy(): Promise<void> {
    return this.runSerialized(async () => {
      await this.disconnectActiveRoom();
      this.clearTrackedState();
      this.lastError = null;
      this.connectionStatus = "disconnected";
      this.subscribers.clear();
    });
  }

  private async setMicrophoneEnabledInternal(enabled: boolean): Promise<void> {
    const room = this.activeRoom;
    if (!room) {
      throw new RtcClientError("not_connected", "Cannot toggle microphone while not connected.");
    }
    try {
      await room.localParticipant.setMicrophoneEnabled(enabled);
      this.isMicrophoneEnabled = enabled;
      this.lastError = null;
      this.emitSnapshot();
    } catch (error) {
      const message = sanitizeErrorMessage(error);
      this.lastError = {
        code: "microphone_toggle_failed",
        message: `Unable to update microphone state: ${message}`,
      };
      this.emitSnapshot();
      throw new RtcClientError("microphone_toggle_failed", this.lastError.message);
    }
  }

  private runSerialized<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.queuedOperation.then(operation, operation);
    this.queuedOperation = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  private emitSnapshot(): void {
    const snapshot = this.snapshot();
    for (const listener of this.subscribers) {
      try {
        listener(snapshot);
      } catch {
        // Subscriber errors are isolated to avoid breaking RTC state updates.
      }
    }
  }

  private bindRoomListeners(room: RtcRoomLike): void {
    const register = (event: RoomEvent, listener: RoomListener) => {
      room.on(event, listener);
      this.roomListeners.push({ event, listener });
    };

    register(RoomEvent.ConnectionStateChanged, (state) => {
      if (room !== this.activeRoom || !isConnectionState(state)) {
        return;
      }
      this.connectionStatus = toRtcConnectionStatus(state);
      if (state === ConnectionState.Disconnected) {
        this.clearTrackedState();
      }
      this.emitSnapshot();
    });

    register(RoomEvent.ParticipantConnected, (participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.upsertParticipant(participant);
      this.emitSnapshot();
    });

    register(RoomEvent.ParticipantDisconnected, (participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      const identity = readIdentity(participant);
      if (!identity) {
        return;
      }
      this.participants.delete(identity);
      this.emitSnapshot();
    });

    register(RoomEvent.TrackSubscribed, (_track, publication, participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.addTrackSubscription(participant, publication);
      this.emitSnapshot();
    });

    register(RoomEvent.TrackUnsubscribed, (_track, publication, participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.removeTrackSubscription(participant, publication);
      this.emitSnapshot();
    });

    register(RoomEvent.Disconnected, () => {
      if (room !== this.activeRoom) {
        return;
      }
      this.connectionStatus = "disconnected";
      this.clearTrackedState();
      this.emitSnapshot();
    });
  }

  private isRemoteParticipantLike(value: unknown): value is RemoteParticipantLike {
    if (!value || typeof value !== "object") {
      return false;
    }
    const candidate = value as { trackPublications?: unknown };
    return candidate.trackPublications instanceof Map;
  }

  private upsertParticipant(participant: RemoteParticipantLike): void {
    const identity = readIdentity(participant);
    if (!identity) {
      return;
    }

    const existing = this.participants.get(identity);
    if (!existing && this.participants.size >= this.maxParticipants) {
      return;
    }

    const trackSids = collectTrackSids(participant.trackPublications, this.maxTracksPerParticipant);
    this.participants.set(identity, {
      identity,
      trackSids,
    });
  }

  private addTrackSubscription(participant: RemoteParticipantLike, publication: unknown): void {
    const identity = readIdentity(participant);
    const trackSid = readTrackSid(publication);
    if (!identity || !trackSid) {
      return;
    }

    let tracked = this.participants.get(identity);
    if (!tracked) {
      if (this.participants.size >= this.maxParticipants) {
        return;
      }
      tracked = {
        identity,
        trackSids: new Set<string>(),
      };
      this.participants.set(identity, tracked);
    }

    if (tracked.trackSids.size >= this.maxTracksPerParticipant) {
      return;
    }
    tracked.trackSids.add(trackSid);
  }

  private removeTrackSubscription(participant: RemoteParticipantLike, publication: unknown): void {
    const identity = readIdentity(participant);
    const trackSid = readTrackSid(publication);
    if (!identity || !trackSid) {
      return;
    }
    const tracked = this.participants.get(identity);
    if (!tracked) {
      return;
    }
    tracked.trackSids.delete(trackSid);
  }

  private seedParticipants(room: RtcRoomLike): void {
    for (const participant of room.remoteParticipants.values()) {
      this.upsertParticipant(participant);
    }
  }

  private clearTrackedState(): void {
    this.participants.clear();
    this.localParticipantIdentity = null;
    this.isMicrophoneEnabled = false;
  }

  private async disconnectActiveRoom(): Promise<void> {
    const room = this.activeRoom;
    if (!room) {
      return;
    }
    this.activeRoom = null;
    await this.disconnectRoom(room);
  }

  private async disconnectRoom(room: RtcRoomLike): Promise<void> {
    this.unbindRoomListeners(room);
    try {
      await room.disconnect(true);
    } catch {
      // Disconnect errors are intentionally swallowed during teardown.
    }
  }

  private unbindRoomListeners(room: RtcRoomLike): void {
    for (const registered of this.roomListeners) {
      room.off(registered.event, registered.listener);
    }
    this.roomListeners = [];
  }
}

export function createRtcClient(options: CreateRtcClientOptions = {}): RtcClient {
  return new RtcClientImpl(options);
}
