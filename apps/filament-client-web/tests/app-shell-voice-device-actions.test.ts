import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import type {
  AudioDeviceOption,
  MediaDeviceId,
  VoiceDevicePreferences,
} from "../src/lib/voice-device-settings";
import { createVoiceDeviceActions } from "../src/features/app-shell/runtime/voice-device-actions";

const MIC_ONE: AudioDeviceOption = {
  kind: "audioinput",
  deviceId: "mic-1" as MediaDeviceId,
  label: "Microphone 1",
};

const SPEAKER_ONE: AudioDeviceOption = {
  kind: "audiooutput",
  deviceId: "spk-1" as MediaDeviceId,
  label: "Speaker 1",
};

function createVoiceDeviceActionsHarness(input?: {
  initialPreferences?: VoiceDevicePreferences;
  initialAudioInputs?: AudioDeviceOption[];
  initialAudioOutputs?: AudioDeviceOption[];
  isVoiceSessionActive?: boolean;
}) {
  const [voiceDevicePreferences, setVoiceDevicePreferences] = createSignal<VoiceDevicePreferences>(
    input?.initialPreferences ?? {
      audioInputDeviceId: null,
      audioOutputDeviceId: null,
    },
  );
  const [audioInputDevices, setAudioInputDevices] = createSignal<AudioDeviceOption[]>(
    input?.initialAudioInputs ?? [],
  );
  const [audioOutputDevices, setAudioOutputDevices] = createSignal<AudioDeviceOption[]>(
    input?.initialAudioOutputs ?? [],
  );
  const [isRefreshingAudioDevices, setRefreshingAudioDevices] = createSignal(false);
  const [audioDevicesStatus, setAudioDevicesStatus] = createSignal("");
  const [audioDevicesError, setAudioDevicesError] = createSignal("");
  const [isVoiceSessionActive] = createSignal(Boolean(input?.isVoiceSessionActive));

  const rtcClient = {
    setAudioInputDevice: vi.fn(async () => undefined),
    setAudioOutputDevice: vi.fn(async () => undefined),
  };

  const saveVoiceDevicePreferences = vi.fn();
  const enumerateAudioDevices = vi.fn(async () => ({
    audioInputs: audioInputDevices(),
    audioOutputs: audioOutputDevices(),
  }));
  const canRequestAudioCapturePermission = vi.fn(() => false);
  const requestAudioCapturePermission = vi.fn(async () => undefined);
  const reconcileVoiceDevicePreferences = vi.fn(
    (preferences: VoiceDevicePreferences) => preferences,
  );
  const unavailableVoiceDeviceError = vi.fn((kind: "audioinput" | "audiooutput") =>
    `unavailable-${kind}`,
  );
  const resolveVoiceDevicePreferenceStatus = vi.fn(
    (_kind: "audioinput" | "audiooutput", appliedToRtcClient: boolean) =>
      appliedToRtcClient ? "applied-remote" : "saved-local",
  );
  const mapError = vi.fn(() => "mapped enumerate error");
  const mapRtcError = vi.fn(() => "mapped rtc error");

  const actions = createVoiceDeviceActions(
    {
      voiceDevicePreferences,
      setVoiceDevicePreferences,
      audioInputDevices,
      audioOutputDevices,
      isRefreshingAudioDevices,
      setRefreshingAudioDevices,
      setAudioInputDevices,
      setAudioOutputDevices,
      setAudioDevicesStatus,
      setAudioDevicesError,
      isVoiceSessionActive,
      peekRtcClient: () => rtcClient,
    },
    {
      saveVoiceDevicePreferences,
      enumerateAudioDevices,
      canRequestAudioCapturePermission,
      requestAudioCapturePermission,
      reconcileVoiceDevicePreferences,
      unavailableVoiceDeviceError,
      resolveVoiceDevicePreferenceStatus,
      mapError,
      mapRtcError,
    },
  );

  return {
    actions,
    rtcClient,
    state: {
      voiceDevicePreferences,
      audioInputDevices,
      audioOutputDevices,
      isRefreshingAudioDevices,
      audioDevicesStatus,
      audioDevicesError,
    },
    deps: {
      saveVoiceDevicePreferences,
      enumerateAudioDevices,
      canRequestAudioCapturePermission,
      requestAudioCapturePermission,
      reconcileVoiceDevicePreferences,
      unavailableVoiceDeviceError,
      resolveVoiceDevicePreferenceStatus,
      mapError,
      mapRtcError,
    },
  };
}

describe("app shell voice device actions", () => {
  it("requests microphone permission and refreshes inventory when prompted", async () => {
    const harness = createVoiceDeviceActionsHarness({
      initialAudioInputs: [],
      initialAudioOutputs: [SPEAKER_ONE],
    });

    harness.deps.canRequestAudioCapturePermission.mockReturnValue(true);
    harness.deps.enumerateAudioDevices
      .mockResolvedValueOnce({
        audioInputs: [],
        audioOutputs: [SPEAKER_ONE],
      })
      .mockResolvedValueOnce({
        audioInputs: [MIC_ONE],
        audioOutputs: [SPEAKER_ONE],
      });

    await harness.actions.refreshAudioDeviceInventory(true);

    expect(harness.deps.requestAudioCapturePermission).toHaveBeenCalledTimes(1);
    expect(harness.deps.enumerateAudioDevices).toHaveBeenCalledTimes(2);
    expect(harness.state.audioInputDevices()).toEqual([MIC_ONE]);
    expect(harness.state.audioOutputDevices()).toEqual([SPEAKER_ONE]);
    expect(harness.state.audioDevicesStatus()).toBe(
      "Detected 1 microphone(s) and 1 speaker(s).",
    );
    expect(harness.state.audioDevicesError()).toBe("");
  });

  it("rejects unavailable device selections", async () => {
    const harness = createVoiceDeviceActionsHarness({
      initialAudioInputs: [MIC_ONE],
      initialAudioOutputs: [SPEAKER_ONE],
    });

    await harness.actions.setVoiceDevicePreference("audioinput", "missing-device");

    expect(harness.deps.unavailableVoiceDeviceError).toHaveBeenCalledWith("audioinput");
    expect(harness.deps.saveVoiceDevicePreferences).not.toHaveBeenCalled();
    expect(harness.state.audioDevicesError()).toBe("unavailable-audioinput");
  });

  it("stores preference locally and sets status when voice session is inactive", async () => {
    const harness = createVoiceDeviceActionsHarness({
      initialAudioInputs: [MIC_ONE],
      initialAudioOutputs: [SPEAKER_ONE],
      isVoiceSessionActive: false,
    });

    await harness.actions.setVoiceDevicePreference("audioinput", MIC_ONE.deviceId);

    expect(harness.deps.saveVoiceDevicePreferences).toHaveBeenCalledTimes(1);
    expect(harness.deps.resolveVoiceDevicePreferenceStatus).toHaveBeenCalledWith(
      "audioinput",
      false,
      MIC_ONE.deviceId,
    );
    expect(harness.rtcClient.setAudioInputDevice).not.toHaveBeenCalled();
    expect(harness.state.audioDevicesStatus()).toBe("saved-local");
    expect(harness.state.voiceDevicePreferences().audioInputDeviceId).toBe(MIC_ONE.deviceId);
  });
});
