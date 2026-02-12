import { DomainValidationError } from "../domain/auth";
import {
  audioDeviceEnumerationUnavailableMessage,
  mediaContextHint,
} from "./browser-context";

export type AudioDeviceKind = "audioinput" | "audiooutput";
export type MediaDeviceId = string & { readonly __brand: "media_device_id" };

export interface VoiceDevicePreferences {
  audioInputDeviceId: MediaDeviceId | null;
  audioOutputDeviceId: MediaDeviceId | null;
}

export interface AudioDeviceOption {
  kind: AudioDeviceKind;
  deviceId: MediaDeviceId;
  label: string;
}

export interface AudioDeviceInventory {
  audioInputs: AudioDeviceOption[];
  audioOutputs: AudioDeviceOption[];
}

export const VOICE_DEVICE_SETTINGS_STORAGE_KEY = "filament.voice.settings.v1";

const MAX_STORAGE_BYTES = 8_192;
const MAX_DEVICE_ID_CHARS = 512;
const MAX_DEVICE_LABEL_CHARS = 160;
const MAX_DEVICES_PER_KIND = 64;
const AUDIO_PERMISSION_DENIED_ERROR_NAMES = new Set([
  "notallowederror",
  "securityerror",
  "permissiondeniederror",
]);
const AUDIO_DEVICE_NOT_FOUND_ERROR_NAMES = new Set([
  "notfounderror",
  "devicesnotfounderror",
]);
const AUDIO_DEVICE_UNAVAILABLE_ERROR_NAMES = new Set([
  "notreadableerror",
  "trackstarterror",
  "aborterror",
]);

const DEFAULT_VOICE_DEVICE_PREFERENCES: VoiceDevicePreferences = {
  audioInputDeviceId: null,
  audioOutputDeviceId: null,
};

function canUseStorage(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.localStorage !== "undefined" &&
    typeof window.localStorage.getItem === "function" &&
    typeof window.localStorage.setItem === "function"
  );
}

function hasControlCharacters(value: string): boolean {
  for (const char of value) {
    const code = char.charCodeAt(0);
    if (code < 0x20 || code === 0x7f) {
      return true;
    }
  }
  return false;
}

export function mediaDeviceIdFromInput(input: string): MediaDeviceId {
  if (input.length < 1 || input.length > MAX_DEVICE_ID_CHARS) {
    throw new DomainValidationError("Audio device ID has invalid length.");
  }
  if (hasControlCharacters(input)) {
    throw new DomainValidationError("Audio device ID contains invalid characters.");
  }
  return input as MediaDeviceId;
}

function mediaDeviceLabelFromInput(input: string): string {
  const value = input.trim();
  if (value.length < 1 || value.length > MAX_DEVICE_LABEL_CHARS || hasControlCharacters(value)) {
    throw new DomainValidationError("Audio device label is invalid.");
  }
  return value;
}

function parseOptionalDeviceId(value: unknown): MediaDeviceId | null {
  if (value === null || typeof value === "undefined") {
    return null;
  }
  if (typeof value !== "string") {
    throw new DomainValidationError("Audio device preference must be a string or null.");
  }
  if (value.length === 0) {
    return null;
  }
  return mediaDeviceIdFromInput(value);
}

export function defaultVoiceDevicePreferences(): VoiceDevicePreferences {
  return { ...DEFAULT_VOICE_DEVICE_PREFERENCES };
}

export function loadVoiceDevicePreferences(): VoiceDevicePreferences {
  if (!canUseStorage()) {
    return defaultVoiceDevicePreferences();
  }
  const raw = window.localStorage.getItem(VOICE_DEVICE_SETTINGS_STORAGE_KEY);
  if (!raw || raw.length > MAX_STORAGE_BYTES) {
    return defaultVoiceDevicePreferences();
  }
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") {
      return defaultVoiceDevicePreferences();
    }
    const data = parsed as {
      audioInputDeviceId?: unknown;
      audioOutputDeviceId?: unknown;
    };
    return {
      audioInputDeviceId: parseOptionalDeviceId(data.audioInputDeviceId),
      audioOutputDeviceId: parseOptionalDeviceId(data.audioOutputDeviceId),
    };
  } catch {
    return defaultVoiceDevicePreferences();
  }
}

export function saveVoiceDevicePreferences(preferences: VoiceDevicePreferences): void {
  if (!canUseStorage()) {
    return;
  }
  window.localStorage.setItem(
    VOICE_DEVICE_SETTINGS_STORAGE_KEY,
    JSON.stringify({
      audioInputDeviceId: preferences.audioInputDeviceId,
      audioOutputDeviceId: preferences.audioOutputDeviceId,
    }),
  );
}

function buildFallbackDeviceLabel(kind: AudioDeviceKind, index: number): string {
  if (kind === "audioinput") {
    return `Microphone ${index + 1}`;
  }
  return `Speaker ${index + 1}`;
}

function appendDevice(
  map: Map<string, AudioDeviceOption>,
  raw: MediaDeviceInfo,
  kind: AudioDeviceKind,
): void {
  if (map.size >= MAX_DEVICES_PER_KIND) {
    return;
  }

  let deviceId: MediaDeviceId;
  try {
    deviceId = mediaDeviceIdFromInput(raw.deviceId);
  } catch {
    return;
  }
  if (map.has(deviceId)) {
    return;
  }

  let label: string;
  try {
    label = mediaDeviceLabelFromInput(raw.label);
  } catch {
    label = buildFallbackDeviceLabel(kind, map.size);
  }

  map.set(deviceId, {
    kind,
    deviceId,
    label,
  });
}

export async function enumerateAudioDevices(): Promise<AudioDeviceInventory> {
  if (
    typeof navigator === "undefined" ||
    typeof navigator.mediaDevices === "undefined" ||
    typeof navigator.mediaDevices.enumerateDevices !== "function"
  ) {
    throw new DomainValidationError(audioDeviceEnumerationUnavailableMessage());
  }

  let devices: MediaDeviceInfo[];
  try {
    devices = await navigator.mediaDevices.enumerateDevices();
  } catch {
    const hint = mediaContextHint();
    if (hint) {
      throw new DomainValidationError(`Unable to enumerate audio devices. ${hint}`);
    }
    throw new DomainValidationError("Unable to enumerate audio devices.");
  }

  if (!Array.isArray(devices)) {
    throw new DomainValidationError("Audio device enumeration returned an invalid response.");
  }

  const audioInputsById = new Map<string, AudioDeviceOption>();
  const audioOutputsById = new Map<string, AudioDeviceOption>();

  for (const device of devices) {
    if (!device || typeof device !== "object") {
      continue;
    }
    if (device.kind === "audioinput") {
      appendDevice(audioInputsById, device, "audioinput");
      continue;
    }
    if (device.kind === "audiooutput") {
      appendDevice(audioOutputsById, device, "audiooutput");
    }
  }

  return {
    audioInputs: [...audioInputsById.values()],
    audioOutputs: [...audioOutputsById.values()],
  };
}

export function canRequestAudioCapturePermission(): boolean {
  return (
    typeof navigator !== "undefined" &&
    typeof navigator.mediaDevices !== "undefined" &&
    typeof navigator.mediaDevices.getUserMedia === "function"
  );
}

function mediaPermissionErrorMessage(error: unknown): string {
  const rawName =
    error && typeof error === "object" && typeof (error as { name?: unknown }).name === "string"
      ? (error as { name: string }).name
      : "";
  const normalizedName = rawName.trim().toLowerCase();
  const hint = mediaContextHint();

  if (AUDIO_PERMISSION_DENIED_ERROR_NAMES.has(normalizedName)) {
    if (hint) {
      return `Microphone permission prompt is unavailable in this browser context. ${hint}`;
    }
    return "Microphone permission was denied. Allow access in browser site settings and retry.";
  }
  if (AUDIO_DEVICE_NOT_FOUND_ERROR_NAMES.has(normalizedName)) {
    return "No microphone device is available on this system.";
  }
  if (AUDIO_DEVICE_UNAVAILABLE_ERROR_NAMES.has(normalizedName)) {
    return "Microphone is unavailable. It may be in use by another app or blocked by OS privacy settings.";
  }

  if (hint) {
    return `Unable to request microphone permission. ${hint}`;
  }
  return "Unable to request microphone permission.";
}

export async function requestAudioCapturePermission(): Promise<void> {
  if (!canRequestAudioCapturePermission()) {
    throw new DomainValidationError("Microphone permission request is unavailable in this browser.");
  }

  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: true,
      video: false,
    });
  } catch (error) {
    throw new DomainValidationError(mediaPermissionErrorMessage(error));
  }

  if (!stream || typeof stream.getTracks !== "function") {
    throw new DomainValidationError("Microphone permission request returned an invalid stream.");
  }

  const tracks = stream.getTracks();
  for (const track of tracks) {
    if (track && typeof track.stop === "function") {
      track.stop();
    }
  }
}

export function reconcileVoiceDevicePreferences(
  preferences: VoiceDevicePreferences,
  inventory: AudioDeviceInventory,
): VoiceDevicePreferences {
  const inputIds = new Set(inventory.audioInputs.map((device) => device.deviceId));
  const outputIds = new Set(inventory.audioOutputs.map((device) => device.deviceId));

  return {
    audioInputDeviceId:
      preferences.audioInputDeviceId && inputIds.has(preferences.audioInputDeviceId)
        ? preferences.audioInputDeviceId
        : null,
    audioOutputDeviceId:
      preferences.audioOutputDeviceId && outputIds.has(preferences.audioOutputDeviceId)
        ? preferences.audioOutputDeviceId
        : null,
  };
}
