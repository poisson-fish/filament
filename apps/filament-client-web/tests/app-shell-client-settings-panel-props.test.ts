import { describe, expect, it, vi } from "vitest";
import type { SettingsPanelBuilderOptions } from "../src/features/app-shell/adapters/panel-host-props";
import {
  defaultVoiceDevicePreferences,
  mediaDeviceIdFromInput,
} from "../src/lib/voice-device-settings";
import { createClientSettingsPanelProps } from "../src/features/app-shell/runtime/client-settings-panel-props";

describe("app shell client settings panel props", () => {
  it("maps settings values and handlers", async () => {
    const onOpenSettingsCategory = vi.fn();
    const onOpenVoiceSettingsSubmenu = vi.fn();
    const onSetVoiceDevicePreference = vi.fn();
    const onRefreshAudioDeviceInventory = vi.fn();
    const onSaveProfileSettings = vi.fn();
    const onUploadProfileAvatar = vi.fn();

    const panelProps = createClientSettingsPanelProps({
      activeSettingsCategory: "profile",
      activeVoiceSettingsSubmenu: "audio-devices",
      voiceDevicePreferences: {
        audioInputDeviceId: mediaDeviceIdFromInput("input-1"),
        audioOutputDeviceId: mediaDeviceIdFromInput("output-1"),
      },
      audioInputDevices: [
        {
          deviceId: mediaDeviceIdFromInput("input-1"),
          label: "Mic",
          kind: "audioinput",
        },
      ],
      audioOutputDevices: [
        {
          deviceId: mediaDeviceIdFromInput("output-1"),
          label: "Speaker",
          kind: "audiooutput",
        },
      ],
      isRefreshingAudioDevices: false,
      audioDevicesStatus: "ready",
      audioDevicesError: "",
      profile: {
        userId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      } as unknown as NonNullable<SettingsPanelBuilderOptions["profile"]>,
      profileDraftUsername: "user",
      profileDraftAbout: "about",
      selectedAvatarFilename: "avatar.png",
      isSavingProfile: false,
      isUploadingProfileAvatar: false,
      profileSettingsStatus: "saved",
      profileSettingsError: "",
      onOpenSettingsCategory,
      onOpenVoiceSettingsSubmenu,
      onSetVoiceDevicePreference,
      onRefreshAudioDeviceInventory,
      setProfileDraftUsername: () => undefined,
      setProfileDraftAbout: () => undefined,
      setSelectedProfileAvatarFile: () => undefined,
      onSaveProfileSettings,
      onUploadProfileAvatar,
      avatarUrlForUser: (userId) => `/avatar/${userId}`,
    });

    expect(panelProps.activeSettingsCategory).toBe("profile");
    expect(panelProps.audioInputDevices).toHaveLength(1);
    expect(panelProps.profileAvatarUrl).toBe("/avatar/01ARZ3NDEKTSV4RRFFQ69G5FAV");

    panelProps.onOpenSettingsCategory("voice");
    expect(onOpenSettingsCategory).toHaveBeenCalledWith("voice");

    panelProps.onOpenVoiceSettingsSubmenu("audio-devices");
    expect(onOpenVoiceSettingsSubmenu).toHaveBeenCalledWith("audio-devices");

    await panelProps.onSetVoiceDevicePreference("audioinput", "input-2");
    expect(onSetVoiceDevicePreference).toHaveBeenCalledWith("audioinput", "input-2");

    await panelProps.onRefreshAudioDeviceInventory();
    expect(onRefreshAudioDeviceInventory).toHaveBeenCalledTimes(1);

    await panelProps.onSaveProfileSettings();
    expect(onSaveProfileSettings).toHaveBeenCalledTimes(1);

    await panelProps.onUploadProfileAvatar();
    expect(onUploadProfileAvatar).toHaveBeenCalledTimes(1);
  });

  it("keeps profile avatar url null when no profile is selected", () => {
    const avatarUrlForUser = vi.fn();

    const panelProps = createClientSettingsPanelProps({
      activeSettingsCategory: "voice",
      activeVoiceSettingsSubmenu: "audio-devices",
      voiceDevicePreferences: defaultVoiceDevicePreferences(),
      audioInputDevices: [],
      audioOutputDevices: [],
      isRefreshingAudioDevices: false,
      audioDevicesStatus: "",
      audioDevicesError: "",
      profile: null,
      profileDraftUsername: "",
      profileDraftAbout: "",
      selectedAvatarFilename: "",
      isSavingProfile: false,
      isUploadingProfileAvatar: false,
      profileSettingsStatus: "",
      profileSettingsError: "",
      onOpenSettingsCategory: () => undefined,
      onOpenVoiceSettingsSubmenu: () => undefined,
      onSetVoiceDevicePreference: () => undefined,
      onRefreshAudioDeviceInventory: () => undefined,
      setProfileDraftUsername: () => undefined,
      setProfileDraftAbout: () => undefined,
      setSelectedProfileAvatarFile: () => undefined,
      onSaveProfileSettings: () => undefined,
      onUploadProfileAvatar: () => undefined,
      avatarUrlForUser,
    });

    expect(panelProps.profileAvatarUrl).toBeNull();
    expect(avatarUrlForUser).not.toHaveBeenCalled();
  });
});
