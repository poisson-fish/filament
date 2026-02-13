import type {
  SettingsPanelBuilderOptions,
} from "../adapters/panel-host-props";

export interface ClientSettingsPanelPropsOptions {
  activeSettingsCategory: SettingsPanelBuilderOptions["activeSettingsCategory"];
  activeVoiceSettingsSubmenu: SettingsPanelBuilderOptions["activeVoiceSettingsSubmenu"];
  voiceDevicePreferences: SettingsPanelBuilderOptions["voiceDevicePreferences"];
  audioInputDevices: SettingsPanelBuilderOptions["audioInputDevices"];
  audioOutputDevices: SettingsPanelBuilderOptions["audioOutputDevices"];
  isRefreshingAudioDevices: boolean;
  audioDevicesStatus: string;
  audioDevicesError: string;
  profile: SettingsPanelBuilderOptions["profile"];
  profileDraftUsername: string;
  profileDraftAbout: string;
  selectedAvatarFilename: string;
  isSavingProfile: boolean;
  isUploadingProfileAvatar: boolean;
  profileSettingsStatus: string;
  profileSettingsError: string;
  onOpenSettingsCategory: SettingsPanelBuilderOptions["onOpenSettingsCategory"];
  onOpenVoiceSettingsSubmenu: SettingsPanelBuilderOptions["onOpenVoiceSettingsSubmenu"];
  onSetVoiceDevicePreference: SettingsPanelBuilderOptions["onSetVoiceDevicePreference"];
  onRefreshAudioDeviceInventory: SettingsPanelBuilderOptions["onRefreshAudioDeviceInventory"];
  setProfileDraftUsername: SettingsPanelBuilderOptions["setProfileDraftUsername"];
  setProfileDraftAbout: SettingsPanelBuilderOptions["setProfileDraftAbout"];
  setSelectedProfileAvatarFile: SettingsPanelBuilderOptions["setSelectedProfileAvatarFile"];
  onSaveProfileSettings: SettingsPanelBuilderOptions["onSaveProfileSettings"];
  onUploadProfileAvatar: SettingsPanelBuilderOptions["onUploadProfileAvatar"];
  avatarUrlForUser: (rawUserId: string) => string | null;
}

export function createClientSettingsPanelProps(
  options: ClientSettingsPanelPropsOptions,
): SettingsPanelBuilderOptions {
  return {
    activeSettingsCategory: options.activeSettingsCategory,
    activeVoiceSettingsSubmenu: options.activeVoiceSettingsSubmenu,
    voiceDevicePreferences: options.voiceDevicePreferences,
    audioInputDevices: options.audioInputDevices,
    audioOutputDevices: options.audioOutputDevices,
    isRefreshingAudioDevices: options.isRefreshingAudioDevices,
    audioDevicesStatus: options.audioDevicesStatus,
    audioDevicesError: options.audioDevicesError,
    profile: options.profile,
    profileDraftUsername: options.profileDraftUsername,
    profileDraftAbout: options.profileDraftAbout,
    profileAvatarUrl: options.profile
      ? options.avatarUrlForUser(options.profile.userId)
      : null,
    selectedAvatarFilename: options.selectedAvatarFilename,
    isSavingProfile: options.isSavingProfile,
    isUploadingProfileAvatar: options.isUploadingProfileAvatar,
    profileSettingsStatus: options.profileSettingsStatus,
    profileSettingsError: options.profileSettingsError,
    onOpenSettingsCategory: options.onOpenSettingsCategory,
    onOpenVoiceSettingsSubmenu: options.onOpenVoiceSettingsSubmenu,
    onSetVoiceDevicePreference: options.onSetVoiceDevicePreference,
    onRefreshAudioDeviceInventory: options.onRefreshAudioDeviceInventory,
    setProfileDraftUsername: options.setProfileDraftUsername,
    setProfileDraftAbout: options.setProfileDraftAbout,
    setSelectedProfileAvatarFile: options.setSelectedProfileAvatarFile,
    onSaveProfileSettings: options.onSaveProfileSettings,
    onUploadProfileAvatar: options.onUploadProfileAvatar,
  };
}
