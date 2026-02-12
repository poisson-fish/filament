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
const MAX_ACTIVE_SPEAKERS_CAP = 4_097;
const MAX_AUDIO_DEVICE_ID_CHARS = 512;
const MAX_VIDEO_TRACKS_PER_IDENTITY = 2;

export const RTC_DEFAULT_MAX_PARTICIPANTS = 256;
export const RTC_DEFAULT_MAX_TRACKS_PER_PARTICIPANT = 32;
export const RTC_DEFAULT_ACTIVE_SPEAKER_DEBOUNCE_MS = 180;
export const RTC_DEFAULT_ACTIVE_SPEAKER_HYSTERESIS_MS = 900;

export type RtcConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type RtcErrorCode =
  | "invalid_livekit_url"
  | "invalid_livekit_token"
  | "invalid_audio_device_id"
  | "join_failed"
  | "not_connected"
  | "microphone_toggle_failed"
  | "camera_toggle_failed"
  | "screen_share_toggle_failed"
  | "audio_device_switch_failed"
  | "listener_limit_exceeded";

export interface RtcParticipantSnapshot {
  identity: string;
  subscribedTrackCount: number;
}

export type RtcVideoSource = "camera" | "screen_share";

export interface RtcVideoTrackSnapshot {
  trackSid: string;
  participantIdentity: string;
  source: RtcVideoSource;
  isLocal: boolean;
}

export interface RtcSnapshot {
  connectionStatus: RtcConnectionStatus;
  localParticipantIdentity: string | null;
  isMicrophoneEnabled: boolean;
  isCameraEnabled: boolean;
  isScreenShareEnabled: boolean;
  participants: RtcParticipantSnapshot[];
  videoTracks: RtcVideoTrackSnapshot[];
  activeSpeakerIdentities: string[];
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
  setAudioInputDevice(deviceId: string | null): Promise<void>;
  setAudioOutputDevice(deviceId: string | null): Promise<void>;
  join(request: RtcJoinRequest): Promise<void>;
  leave(): Promise<void>;
  setMicrophoneEnabled(enabled: boolean): Promise<void>;
  toggleMicrophone(): Promise<boolean>;
  setCameraEnabled(enabled: boolean): Promise<void>;
  toggleCamera(): Promise<boolean>;
  setScreenShareEnabled(enabled: boolean): Promise<void>;
  toggleScreenShare(): Promise<boolean>;
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
  source?: unknown;
}

interface RemoteParticipantLike {
  identity?: unknown;
  trackPublications: Map<string, TrackPublicationLike>;
}

interface LocalParticipantLike {
  identity?: unknown;
  isCameraEnabled?: unknown;
  isScreenShareEnabled?: unknown;
  videoTrackPublications?: Map<string, TrackPublicationLike>;
  setMicrophoneEnabled(enabled: boolean): Promise<unknown>;
  setCameraEnabled(enabled: boolean): Promise<unknown>;
  setScreenShareEnabled(enabled: boolean): Promise<unknown>;
}

interface SpeakerParticipantLike {
  identity?: unknown;
}

type RoomListener = (...args: unknown[]) => void;

interface RtcRoomLike {
  state: ConnectionState;
  localParticipant: LocalParticipantLike;
  remoteParticipants: Map<string, RemoteParticipantLike>;
  activeSpeakers?: SpeakerParticipantLike[];
  on(event: RoomEvent, listener: RoomListener): unknown;
  off(event: RoomEvent, listener: RoomListener): unknown;
  connect(url: string, token: string, options?: RoomConnectOptions): Promise<void>;
  disconnect(stopTracks?: boolean): Promise<void>;
  switchActiveDevice(kind: MediaDeviceKind, deviceId: string, exact?: boolean): Promise<boolean>;
}

interface RegisteredRoomListener {
  event: RoomEvent;
  listener: RoomListener;
}

interface TrackedVideoTrack {
  trackSid: string;
  participantIdentity: string;
  source: RtcVideoSource;
  isLocal: boolean;
}

interface CreateRtcClientOptions {
  roomOptions?: RoomOptions;
  connectOptions?: RoomConnectOptions;
  roomFactory?: (options?: RoomOptions) => RtcRoomLike;
  maxParticipants?: number;
  maxTracksPerParticipant?: number;
  activeSpeakerDebounceMs?: number;
  activeSpeakerHysteresisMs?: number;
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
  const sidCandidate = publication as { trackSid?: unknown; sid?: unknown };
  const sid = typeof sidCandidate.trackSid === "string" ? sidCandidate.trackSid : sidCandidate.sid;
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

function sourceFromUnknown(value: unknown): RtcVideoSource | null {
  if (value === "camera") {
    return "camera";
  }
  if (value === "screen_share" || value === "screenShare") {
    return "screen_share";
  }
  return null;
}

function readTrackSource(input: unknown): RtcVideoSource | null {
  if (!input || typeof input !== "object") {
    return null;
  }
  const source = (input as { source?: unknown }).source;
  return sourceFromUnknown(source);
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

function requireBoundedDelayMs(
  value: number | undefined,
  fallback: number,
  label: string,
): number {
  if (value === undefined) {
    return fallback;
  }
  if (!Number.isInteger(value) || value < 0 || value > 60_000) {
    throw new RtcClientError("join_failed", `${label} must be an integer between 0 and 60000.`);
  }
  return value;
}

function normalizeAudioDeviceId(input: string | null | undefined): string | null {
  if (input === null || typeof input === "undefined") {
    return null;
  }
  if (typeof input !== "string") {
    throw new RtcClientError("invalid_audio_device_id", "Audio device ID must be a string.");
  }
  if (input.length < 1 || input.length > MAX_AUDIO_DEVICE_ID_CHARS) {
    throw new RtcClientError(
      "invalid_audio_device_id",
      "Audio device ID is empty or too long.",
    );
  }
  for (const char of input) {
    const code = char.charCodeAt(0);
    if (code < 0x20 || code === 0x7f) {
      throw new RtcClientError(
        "invalid_audio_device_id",
        "Audio device ID contains invalid characters.",
      );
    }
  }
  return input;
}

function audioDeviceLabel(kind: MediaDeviceKind): string {
  if (kind === "audioinput") {
    return "microphone";
  }
  if (kind === "audiooutput") {
    return "speaker";
  }
  return kind;
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
  private readonly activeSpeakerDebounceMs: number;
  private readonly activeSpeakerHysteresisMs: number;
  private readonly subscribers = new Set<(snapshot: RtcSnapshot) => void>();
  private readonly participants = new Map<string, TrackedParticipant>();
  private readonly videoTracksByKey = new Map<string, TrackedVideoTrack>();
  private readonly videoTrackKeyBySid = new Map<string, string>();
  private readonly activeSpeakerIdentities = new Set<string>();
  private readonly pendingSpeakerOnTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private readonly pendingSpeakerOffTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private latestRawActiveSpeakers = new Set<string>();
  private activeRoom: RtcRoomLike | null = null;
  private roomListeners: RegisteredRoomListener[] = [];
  private queuedOperation: Promise<void> = Promise.resolve();
  private connectionStatus: RtcConnectionStatus = "disconnected";
  private localParticipantIdentity: string | null = null;
  private isMicrophoneEnabled = false;
  private isCameraEnabled = false;
  private isScreenShareEnabled = false;
  private preferredAudioInputDeviceId: string | null = null;
  private preferredAudioOutputDeviceId: string | null = null;
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
    this.activeSpeakerDebounceMs = requireBoundedDelayMs(
      options.activeSpeakerDebounceMs,
      RTC_DEFAULT_ACTIVE_SPEAKER_DEBOUNCE_MS,
      "activeSpeakerDebounceMs",
    );
    this.activeSpeakerHysteresisMs = requireBoundedDelayMs(
      options.activeSpeakerHysteresisMs,
      RTC_DEFAULT_ACTIVE_SPEAKER_HYSTERESIS_MS,
      "activeSpeakerHysteresisMs",
    );
  }

  snapshot(): RtcSnapshot {
    const participants = Array.from(this.participants.values())
      .map((participant) => ({
        identity: participant.identity,
        subscribedTrackCount: participant.trackSids.size,
      }))
      .sort((left, right) => left.identity.localeCompare(right.identity));
    const videoTracks = Array.from(this.videoTracksByKey.values()).sort((left, right) => {
      if (left.isLocal !== right.isLocal) {
        return left.isLocal ? -1 : 1;
      }
      if (left.participantIdentity !== right.participantIdentity) {
        return left.participantIdentity.localeCompare(right.participantIdentity);
      }
      if (left.source !== right.source) {
        return left.source === "camera" ? -1 : 1;
      }
      return left.trackSid.localeCompare(right.trackSid);
    });

    return {
      connectionStatus: this.connectionStatus,
      localParticipantIdentity: this.localParticipantIdentity,
      isMicrophoneEnabled: this.isMicrophoneEnabled,
      isCameraEnabled: this.isCameraEnabled,
      isScreenShareEnabled: this.isScreenShareEnabled,
      participants,
      videoTracks,
      activeSpeakerIdentities: [...this.activeSpeakerIdentities].sort((left, right) =>
        left.localeCompare(right),
      ),
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

  setAudioInputDevice(deviceId: string | null): Promise<void> {
    return this.runSerialized(async () => {
      this.preferredAudioInputDeviceId = normalizeAudioDeviceId(deviceId);
      const room = this.activeRoom;
      if (!room || !this.preferredAudioInputDeviceId) {
        return;
      }
      await this.switchRoomAudioDevice(room, "audioinput", this.preferredAudioInputDeviceId);
      this.lastError = null;
      this.emitSnapshot();
    });
  }

  setAudioOutputDevice(deviceId: string | null): Promise<void> {
    return this.runSerialized(async () => {
      this.preferredAudioOutputDeviceId = normalizeAudioDeviceId(deviceId);
      const room = this.activeRoom;
      if (!room || !this.preferredAudioOutputDeviceId) {
        return;
      }
      await this.switchRoomAudioDevice(room, "audiooutput", this.preferredAudioOutputDeviceId);
      this.lastError = null;
      this.emitSnapshot();
    });
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
      this.refreshLocalMediaState(room);
      this.reconcileActiveSpeakerState(this.collectSpeakerIdentities(room.activeSpeakers ?? []));
      try {
        await this.applyPreferredAudioDevices(room);
        this.lastError = null;
      } catch (error) {
        const message = sanitizeErrorMessage(error);
        this.lastError = {
          code: "audio_device_switch_failed",
          message: `Connected, but unable to apply selected audio device: ${message}`,
        };
      }
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

  setCameraEnabled(enabled: boolean): Promise<void> {
    return this.runSerialized(async () => {
      await this.setCameraEnabledInternal(enabled);
    });
  }

  toggleCamera(): Promise<boolean> {
    return this.runSerialized(async () => {
      const next = !this.isCameraEnabled;
      await this.setCameraEnabledInternal(next);
      return next;
    });
  }

  setScreenShareEnabled(enabled: boolean): Promise<void> {
    return this.runSerialized(async () => {
      await this.setScreenShareEnabledInternal(enabled);
    });
  }

  toggleScreenShare(): Promise<boolean> {
    return this.runSerialized(async () => {
      const next = !this.isScreenShareEnabled;
      await this.setScreenShareEnabledInternal(next);
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

  private async setCameraEnabledInternal(enabled: boolean): Promise<void> {
    const room = this.activeRoom;
    if (!room) {
      throw new RtcClientError("not_connected", "Cannot toggle camera while not connected.");
    }
    try {
      await room.localParticipant.setCameraEnabled(enabled);
      this.refreshLocalMediaState(room);
      this.lastError = null;
      this.emitSnapshot();
    } catch (error) {
      const message = sanitizeErrorMessage(error);
      this.lastError = {
        code: "camera_toggle_failed",
        message: `Unable to update camera state: ${message}`,
      };
      this.emitSnapshot();
      throw new RtcClientError("camera_toggle_failed", this.lastError.message);
    }
  }

  private async setScreenShareEnabledInternal(enabled: boolean): Promise<void> {
    const room = this.activeRoom;
    if (!room) {
      throw new RtcClientError("not_connected", "Cannot toggle screen share while not connected.");
    }
    try {
      await room.localParticipant.setScreenShareEnabled(enabled);
      this.refreshLocalMediaState(room);
      this.lastError = null;
      this.emitSnapshot();
    } catch (error) {
      const message = sanitizeErrorMessage(error);
      this.lastError = {
        code: "screen_share_toggle_failed",
        message: `Unable to update screen share state: ${message}`,
      };
      this.emitSnapshot();
      throw new RtcClientError("screen_share_toggle_failed", this.lastError.message);
    }
  }

  private async applyPreferredAudioDevices(room: RtcRoomLike): Promise<void> {
    if (this.preferredAudioInputDeviceId) {
      await this.switchRoomAudioDevice(room, "audioinput", this.preferredAudioInputDeviceId);
    }
    if (this.preferredAudioOutputDeviceId) {
      await this.switchRoomAudioDevice(room, "audiooutput", this.preferredAudioOutputDeviceId);
    }
  }

  private async switchRoomAudioDevice(
    room: RtcRoomLike,
    kind: MediaDeviceKind,
    deviceId: string,
  ): Promise<void> {
    try {
      const switched = await room.switchActiveDevice(kind, deviceId, true);
      if (!switched) {
        throw new Error(`selected ${audioDeviceLabel(kind)} is unavailable`);
      }
    } catch (error) {
      const message = sanitizeErrorMessage(error);
      this.lastError = {
        code: "audio_device_switch_failed",
        message: `Unable to switch ${audioDeviceLabel(kind)}: ${message}`,
      };
      this.emitSnapshot();
      throw new RtcClientError("audio_device_switch_failed", this.lastError.message);
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
      this.removeVideoTracksForIdentity(identity, false);
      this.removeActiveSpeakerIdentity(identity);
      this.emitSnapshot();
    });

    register(RoomEvent.TrackSubscribed, (track, publication, participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.addTrackSubscription(participant, publication);
      this.upsertRemoteVideoTrack(participant, publication, track);
      this.emitSnapshot();
    });

    register(RoomEvent.TrackUnsubscribed, (_track, publication, participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.removeTrackSubscription(participant, publication);
      this.removeVideoTrackByPublication(publication);
      this.emitSnapshot();
    });

    register(RoomEvent.TrackUnpublished, (publication, participant) => {
      if (room !== this.activeRoom || !this.isRemoteParticipantLike(participant)) {
        return;
      }
      this.removeVideoTrackByPublication(publication);
      this.emitSnapshot();
    });

    register(RoomEvent.LocalTrackPublished, () => {
      if (room !== this.activeRoom) {
        return;
      }
      this.refreshLocalMediaState(room);
      this.emitSnapshot();
    });

    register(RoomEvent.LocalTrackUnpublished, () => {
      if (room !== this.activeRoom) {
        return;
      }
      this.refreshLocalMediaState(room);
      this.emitSnapshot();
    });

    register(RoomEvent.ActiveSpeakersChanged, (speakers) => {
      if (room !== this.activeRoom || !Array.isArray(speakers)) {
        return;
      }
      this.reconcileActiveSpeakerState(this.collectSpeakerIdentities(speakers));
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

  private upsertRemoteVideoTrack(
    participant: RemoteParticipantLike,
    publication: unknown,
    track?: unknown,
  ): void {
    const identity = readIdentity(participant);
    const trackSid = readTrackSid(publication) ?? readTrackSid(track);
    const source = readTrackSource(publication) ?? readTrackSource(track);
    if (!identity || !trackSid || !source) {
      return;
    }
    this.upsertVideoTrack(identity, false, source, trackSid);
  }

  private upsertVideoTrack(
    participantIdentity: string,
    isLocal: boolean,
    source: RtcVideoSource,
    trackSid: string,
  ): void {
    const key = this.videoTrackIdentityKey(participantIdentity, isLocal, source);
    const maxVideoTracks =
      this.maxParticipants * MAX_VIDEO_TRACKS_PER_IDENTITY + MAX_VIDEO_TRACKS_PER_IDENTITY;
    if (!this.videoTracksByKey.has(key) && this.videoTracksByKey.size >= maxVideoTracks) {
      return;
    }
    const existingBySid = this.videoTrackKeyBySid.get(trackSid);
    if (existingBySid && existingBySid !== key) {
      this.videoTracksByKey.delete(existingBySid);
      this.videoTrackKeyBySid.delete(trackSid);
    }
    const previous = this.videoTracksByKey.get(key);
    if (previous && previous.trackSid !== trackSid) {
      this.videoTrackKeyBySid.delete(previous.trackSid);
    }
    this.videoTracksByKey.set(key, {
      trackSid,
      participantIdentity,
      source,
      isLocal,
    });
    this.videoTrackKeyBySid.set(trackSid, key);
  }

  private videoTrackIdentityKey(
    participantIdentity: string,
    isLocal: boolean,
    source: RtcVideoSource,
  ): string {
    return `${isLocal ? "local" : "remote"}|${participantIdentity}|${source}`;
  }

  private removeVideoTrackByPublication(publication: unknown): void {
    const trackSid = readTrackSid(publication);
    if (!trackSid) {
      return;
    }
    this.removeVideoTrackBySid(trackSid);
  }

  private removeVideoTrackBySid(trackSid: string): void {
    const key = this.videoTrackKeyBySid.get(trackSid);
    if (!key) {
      return;
    }
    this.videoTrackKeyBySid.delete(trackSid);
    const tracked = this.videoTracksByKey.get(key);
    if (tracked?.trackSid === trackSid) {
      this.videoTracksByKey.delete(key);
    }
  }

  private removeVideoTracksForIdentity(participantIdentity: string, isLocal: boolean): void {
    const toRemove: string[] = [];
    for (const tracked of this.videoTracksByKey.values()) {
      if (tracked.participantIdentity !== participantIdentity || tracked.isLocal !== isLocal) {
        continue;
      }
      toRemove.push(tracked.trackSid);
    }
    for (const trackSid of toRemove) {
      this.removeVideoTrackBySid(trackSid);
    }
  }

  private hasVideoTrack(
    participantIdentity: string,
    isLocal: boolean,
    source: RtcVideoSource,
  ): boolean {
    return this.videoTracksByKey.has(this.videoTrackIdentityKey(participantIdentity, isLocal, source));
  }

  private refreshLocalMediaState(room: RtcRoomLike): void {
    const localParticipant = room.localParticipant;
    const localIdentity = readIdentity(localParticipant);
    if (!localIdentity) {
      this.isCameraEnabled = false;
      this.isScreenShareEnabled = false;
      return;
    }
    this.removeVideoTracksForIdentity(localIdentity, true);
    if (localParticipant.videoTrackPublications instanceof Map) {
      for (const publication of localParticipant.videoTrackPublications.values()) {
        const trackSid = readTrackSid(publication);
        const source = readTrackSource(publication);
        if (!trackSid || !source) {
          continue;
        }
        this.upsertVideoTrack(localIdentity, true, source, trackSid);
      }
    }
    this.isCameraEnabled = this.readParticipantMediaEnabled(localParticipant.isCameraEnabled)
      ?? this.hasVideoTrack(localIdentity, true, "camera");
    this.isScreenShareEnabled = this.readParticipantMediaEnabled(localParticipant.isScreenShareEnabled)
      ?? this.hasVideoTrack(localIdentity, true, "screen_share");
  }

  private readParticipantMediaEnabled(value: unknown): boolean | null {
    return typeof value === "boolean" ? value : null;
  }

  private seedRemoteVideoTracks(participant: RemoteParticipantLike): void {
    const identity = readIdentity(participant);
    if (!identity) {
      return;
    }
    for (const publication of participant.trackPublications.values()) {
      const trackSid = readTrackSid(publication);
      const source = readTrackSource(publication);
      if (!trackSid || !source) {
        continue;
      }
      this.upsertVideoTrack(identity, false, source, trackSid);
    }
  }

  private seedParticipants(room: RtcRoomLike): void {
    for (const participant of room.remoteParticipants.values()) {
      this.upsertParticipant(participant);
      this.seedRemoteVideoTracks(participant);
    }
  }

  private collectSpeakerIdentities(participants: SpeakerParticipantLike[]): Set<string> {
    const maxActiveSpeakers = Math.min(this.maxParticipants + 1, MAX_ACTIVE_SPEAKERS_CAP);
    const identities = new Set<string>();
    for (const participant of participants) {
      if (identities.size >= maxActiveSpeakers) {
        break;
      }
      const identity = readIdentity(participant);
      if (!identity || !this.isKnownIdentity(identity)) {
        continue;
      }
      identities.add(identity);
    }
    return identities;
  }

  private reconcileActiveSpeakerState(rawSpeakers: Set<string>): void {
    this.latestRawActiveSpeakers = rawSpeakers;

    for (const [identity, timer] of this.pendingSpeakerOnTimers.entries()) {
      if (rawSpeakers.has(identity)) {
        continue;
      }
      clearTimeout(timer);
      this.pendingSpeakerOnTimers.delete(identity);
    }

    for (const [identity, timer] of this.pendingSpeakerOffTimers.entries()) {
      if (!rawSpeakers.has(identity)) {
        continue;
      }
      clearTimeout(timer);
      this.pendingSpeakerOffTimers.delete(identity);
    }

    let changed = false;
    for (const identity of rawSpeakers) {
      if (this.activeSpeakerIdentities.has(identity)) {
        continue;
      }
      if (this.pendingSpeakerOnTimers.has(identity)) {
        continue;
      }
      if (this.activeSpeakerDebounceMs === 0) {
        this.activeSpeakerIdentities.add(identity);
        changed = true;
        continue;
      }
      const timer = setTimeout(() => {
        this.pendingSpeakerOnTimers.delete(identity);
        if (!this.latestRawActiveSpeakers.has(identity) || !this.isKnownIdentity(identity)) {
          return;
        }
        if (!this.activeSpeakerIdentities.has(identity)) {
          this.activeSpeakerIdentities.add(identity);
          this.emitSnapshot();
        }
      }, this.activeSpeakerDebounceMs);
      this.pendingSpeakerOnTimers.set(identity, timer);
    }

    for (const identity of [...this.activeSpeakerIdentities]) {
      if (rawSpeakers.has(identity)) {
        continue;
      }
      if (this.pendingSpeakerOffTimers.has(identity)) {
        continue;
      }
      if (this.activeSpeakerHysteresisMs === 0) {
        this.activeSpeakerIdentities.delete(identity);
        changed = true;
        continue;
      }
      const timer = setTimeout(() => {
        this.pendingSpeakerOffTimers.delete(identity);
        if (this.latestRawActiveSpeakers.has(identity)) {
          return;
        }
        if (this.activeSpeakerIdentities.delete(identity)) {
          this.emitSnapshot();
        }
      }, this.activeSpeakerHysteresisMs);
      this.pendingSpeakerOffTimers.set(identity, timer);
    }

    if (changed) {
      this.emitSnapshot();
    }
  }

  private removeActiveSpeakerIdentity(identity: string): void {
    const pendingOn = this.pendingSpeakerOnTimers.get(identity);
    if (pendingOn) {
      clearTimeout(pendingOn);
      this.pendingSpeakerOnTimers.delete(identity);
    }
    const pendingOff = this.pendingSpeakerOffTimers.get(identity);
    if (pendingOff) {
      clearTimeout(pendingOff);
      this.pendingSpeakerOffTimers.delete(identity);
    }
    this.latestRawActiveSpeakers.delete(identity);
    this.activeSpeakerIdentities.delete(identity);
  }

  private isKnownIdentity(identity: string): boolean {
    return this.localParticipantIdentity === identity || this.participants.has(identity);
  }

  private clearActiveSpeakerTimers(): void {
    for (const timer of this.pendingSpeakerOnTimers.values()) {
      clearTimeout(timer);
    }
    this.pendingSpeakerOnTimers.clear();
    for (const timer of this.pendingSpeakerOffTimers.values()) {
      clearTimeout(timer);
    }
    this.pendingSpeakerOffTimers.clear();
  }

  private clearTrackedState(): void {
    this.clearActiveSpeakerTimers();
    this.latestRawActiveSpeakers.clear();
    this.activeSpeakerIdentities.clear();
    this.participants.clear();
    this.videoTracksByKey.clear();
    this.videoTrackKeyBySid.clear();
    this.localParticipantIdentity = null;
    this.isMicrophoneEnabled = false;
    this.isCameraEnabled = false;
    this.isScreenShareEnabled = false;
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
