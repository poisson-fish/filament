import type { Accessor, Setter } from "solid-js";
import type {
  AudioDeviceOption,
  MediaDeviceId,
  VoiceDevicePreferences,
} from "../../../lib/voice-device-settings";
import {
  canRequestAudioCapturePermission,
  enumerateAudioDevices,
  reconcileVoiceDevicePreferences,
  requestAudioCapturePermission,
  saveVoiceDevicePreferences,
} from "../../../lib/voice-device-settings";
import { mapError, mapRtcError } from "../helpers";
import {
  resolveVoiceDevicePreferenceStatus,
  unavailableVoiceDeviceError,
} from "../controllers/voice-controller";

export type VoiceDevicePreferenceKind = "audioinput" | "audiooutput";

export interface VoiceDeviceRtcClient {
  setAudioInputDevice: (deviceId: MediaDeviceId | null) => Promise<void>;
  setAudioOutputDevice: (deviceId: MediaDeviceId | null) => Promise<void>;
}

export interface VoiceDeviceActionsOptions {
  voiceDevicePreferences: Accessor<VoiceDevicePreferences>;
  setVoiceDevicePreferences: Setter<VoiceDevicePreferences>;
  audioInputDevices: Accessor<AudioDeviceOption[]>;
  audioOutputDevices: Accessor<AudioDeviceOption[]>;
  isRefreshingAudioDevices: Accessor<boolean>;
  setRefreshingAudioDevices: Setter<boolean>;
  setAudioInputDevices: Setter<AudioDeviceOption[]>;
  setAudioOutputDevices: Setter<AudioDeviceOption[]>;
  setAudioDevicesStatus: Setter<string>;
  setAudioDevicesError: Setter<string>;
  isVoiceSessionActive: Accessor<boolean>;
  peekRtcClient: () => VoiceDeviceRtcClient | null;
}

export interface VoiceDeviceActionsDependencies {
  saveVoiceDevicePreferences: (next: VoiceDevicePreferences) => void;
  enumerateAudioDevices: typeof enumerateAudioDevices;
  canRequestAudioCapturePermission: typeof canRequestAudioCapturePermission;
  requestAudioCapturePermission: typeof requestAudioCapturePermission;
  reconcileVoiceDevicePreferences: typeof reconcileVoiceDevicePreferences;
  unavailableVoiceDeviceError: (kind: VoiceDevicePreferenceKind) => string;
  resolveVoiceDevicePreferenceStatus: (
    kind: VoiceDevicePreferenceKind,
    appliedToRtcClient: boolean,
    deviceId: MediaDeviceId | null,
  ) => string;
  mapError: (error: unknown, fallback: string) => string;
  mapRtcError: (error: unknown, fallback: string) => string;
}

const DEFAULT_VOICE_DEVICE_ACTIONS_DEPENDENCIES: VoiceDeviceActionsDependencies = {
  saveVoiceDevicePreferences,
  enumerateAudioDevices,
  canRequestAudioCapturePermission,
  requestAudioCapturePermission,
  reconcileVoiceDevicePreferences,
  unavailableVoiceDeviceError,
  resolveVoiceDevicePreferenceStatus,
  mapError,
  mapRtcError,
};

export function createVoiceDeviceActions(
  options: VoiceDeviceActionsOptions,
  dependencies: Partial<VoiceDeviceActionsDependencies> = {},
) {
  const deps = {
    ...DEFAULT_VOICE_DEVICE_ACTIONS_DEPENDENCIES,
    ...dependencies,
  };

  const persistVoiceDevicePreferences = (next: VoiceDevicePreferences): void => {
    options.setVoiceDevicePreferences(next);
    try {
      deps.saveVoiceDevicePreferences(next);
    } catch {
      options.setAudioDevicesError(
        "Unable to persist audio device preferences in local storage.",
      );
    }
  };

  const refreshAudioDeviceInventory = async (
    requestPermissionPrompt = false,
  ): Promise<void> => {
    if (options.isRefreshingAudioDevices()) {
      return;
    }
    options.setRefreshingAudioDevices(true);
    options.setAudioDevicesError("");
    try {
      let inventory = await deps.enumerateAudioDevices();
      if (
        requestPermissionPrompt &&
        inventory.audioInputs.length === 0 &&
        deps.canRequestAudioCapturePermission()
      ) {
        await deps.requestAudioCapturePermission();
        inventory = await deps.enumerateAudioDevices();
      }
      options.setAudioInputDevices(inventory.audioInputs);
      options.setAudioOutputDevices(inventory.audioOutputs);
      options.setAudioDevicesStatus(
        `Detected ${inventory.audioInputs.length} microphone(s) and ${inventory.audioOutputs.length} speaker(s).`,
      );
      const current = options.voiceDevicePreferences();
      const reconciled = deps.reconcileVoiceDevicePreferences(current, inventory);
      if (
        current.audioInputDeviceId !== reconciled.audioInputDeviceId ||
        current.audioOutputDeviceId !== reconciled.audioOutputDeviceId
      ) {
        persistVoiceDevicePreferences(reconciled);
        options.setAudioDevicesStatus(
          "Some saved audio devices are no longer available. Reverted to system defaults.",
        );
      }
    } catch (error) {
      options.setAudioInputDevices([]);
      options.setAudioOutputDevices([]);
      options.setAudioDevicesStatus("");
      options.setAudioDevicesError(
        deps.mapError(error, "Unable to enumerate audio devices."),
      );
    } finally {
      options.setRefreshingAudioDevices(false);
    }
  };

  const setVoiceDevicePreference = async (
    kind: VoiceDevicePreferenceKind,
    nextValue: string,
  ): Promise<void> => {
    const optionsByKind =
      kind === "audioinput"
        ? options.audioInputDevices()
        : options.audioOutputDevices();
    if (
      nextValue.length > 0 &&
      !optionsByKind.some((entry) => entry.deviceId === nextValue)
    ) {
      options.setAudioDevicesError(deps.unavailableVoiceDeviceError(kind));
      return;
    }

    const nextDeviceId = nextValue.length > 0 ? (nextValue as MediaDeviceId) : null;
    const next: VoiceDevicePreferences =
      kind === "audioinput"
        ? {
            ...options.voiceDevicePreferences(),
            audioInputDeviceId: nextDeviceId,
          }
        : {
            ...options.voiceDevicePreferences(),
            audioOutputDeviceId: nextDeviceId,
          };
    options.setAudioDevicesError("");
    persistVoiceDevicePreferences(next);

    const client = options.peekRtcClient();
    if (!client || !options.isVoiceSessionActive()) {
      options.setAudioDevicesStatus(
        deps.resolveVoiceDevicePreferenceStatus(kind, false, nextDeviceId),
      );
      return;
    }

    try {
      if (kind === "audioinput") {
        await client.setAudioInputDevice(next.audioInputDeviceId);
      } else {
        await client.setAudioOutputDevice(next.audioOutputDeviceId);
      }
      options.setAudioDevicesStatus(
        deps.resolveVoiceDevicePreferenceStatus(kind, true, nextDeviceId),
      );
    } catch (error) {
      options.setAudioDevicesError(
        deps.mapRtcError(
          error,
          kind === "audioinput"
            ? "Unable to apply microphone selection."
            : "Unable to apply speaker selection.",
        ),
      );
    }
  };

  return {
    refreshAudioDeviceInventory,
    setVoiceDevicePreference,
  };
}