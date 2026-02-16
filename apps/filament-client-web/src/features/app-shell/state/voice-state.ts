import { createSignal } from "solid-js";
import { RTC_DISCONNECTED_SNAPSHOT } from "../config/ui-constants";
import type { VoiceSessionCapabilities } from "../types";
import { loadVoiceDevicePreferences, type AudioDeviceOption, type VoiceDevicePreferences } from "../../../lib/voice-device-settings";
import type { RtcSnapshot } from "../../../lib/rtc";
import type { VoiceParticipantPayload } from "../../../lib/gateway";
import {
  createIdleAsyncOperationState,
  type AsyncOperationState,
} from "./async-operation-state";

export const DEFAULT_VOICE_SESSION_CAPABILITIES: VoiceSessionCapabilities = {
  canSubscribe: false,
  publishSources: [],
};

export function createVoiceState() {
  const [rtcSnapshot, setRtcSnapshot] = createSignal<RtcSnapshot>(RTC_DISCONNECTED_SNAPSHOT);
  const [voiceStatus, setVoiceStatus] = createSignal("");
  const [voiceError, setVoiceError] = createSignal("");
  const [isJoiningVoice, setJoiningVoice] = createSignal(false);
  const [voiceJoinState, setVoiceJoinState] = createSignal<AsyncOperationState>(
    createIdleAsyncOperationState(),
  );
  const [isLeavingVoice, setLeavingVoice] = createSignal(false);
  const [isTogglingVoiceMic, setTogglingVoiceMic] = createSignal(false);
  const [isTogglingVoiceCamera, setTogglingVoiceCamera] = createSignal(false);
  const [isTogglingVoiceScreenShare, setTogglingVoiceScreenShare] = createSignal(false);
  const [voiceSessionChannelKey, setVoiceSessionChannelKey] = createSignal<string | null>(null);
  const [voiceSessionStartedAtUnixMs, setVoiceSessionStartedAtUnixMs] = createSignal<number | null>(null);
  const [voiceDurationClockUnixMs, setVoiceDurationClockUnixMs] = createSignal(Date.now());
  const [voiceSessionCapabilities, setVoiceSessionCapabilities] = createSignal<VoiceSessionCapabilities>(
    DEFAULT_VOICE_SESSION_CAPABILITIES,
  );
  const [voiceParticipantsByChannel, setVoiceParticipantsByChannel] = createSignal<
    Record<string, VoiceParticipantPayload[]>
  >({});

  const [voiceDevicePreferences, setVoiceDevicePreferences] = createSignal<VoiceDevicePreferences>(
    loadVoiceDevicePreferences(),
  );
  const [audioInputDevices, setAudioInputDevices] = createSignal<AudioDeviceOption[]>([]);
  const [audioOutputDevices, setAudioOutputDevices] = createSignal<AudioDeviceOption[]>([]);
  const [isRefreshingAudioDevices, setRefreshingAudioDevices] = createSignal(false);
  const [audioDevicesStatus, setAudioDevicesStatus] = createSignal("");
  const [audioDevicesError, setAudioDevicesError] = createSignal("");

  return {
    rtcSnapshot,
    setRtcSnapshot,
    voiceStatus,
    setVoiceStatus,
    voiceError,
    setVoiceError,
    isJoiningVoice,
    setJoiningVoice,
    voiceJoinState,
    setVoiceJoinState,
    isLeavingVoice,
    setLeavingVoice,
    isTogglingVoiceMic,
    setTogglingVoiceMic,
    isTogglingVoiceCamera,
    setTogglingVoiceCamera,
    isTogglingVoiceScreenShare,
    setTogglingVoiceScreenShare,
    voiceSessionChannelKey,
    setVoiceSessionChannelKey,
    voiceSessionStartedAtUnixMs,
    setVoiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs,
    setVoiceDurationClockUnixMs,
    voiceSessionCapabilities,
    setVoiceSessionCapabilities,
    voiceParticipantsByChannel,
    setVoiceParticipantsByChannel,
    voiceDevicePreferences,
    setVoiceDevicePreferences,
    audioInputDevices,
    setAudioInputDevices,
    audioOutputDevices,
    setAudioOutputDevices,
    isRefreshingAudioDevices,
    setRefreshingAudioDevices,
    audioDevicesStatus,
    setAudioDevicesStatus,
    audioDevicesError,
    setAudioDevicesError,
  };
}
