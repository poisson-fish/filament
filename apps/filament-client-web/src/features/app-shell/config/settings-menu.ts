import type {
  SettingsCategory,
  SettingsCategoryItem,
  VoiceSettingsSubmenu,
  VoiceSettingsSubmenuItem,
} from "../types";

export const DEFAULT_SETTINGS_CATEGORY: SettingsCategory = "voice";
export const DEFAULT_VOICE_SETTINGS_SUBMENU: VoiceSettingsSubmenu = "audio-devices";

export const SETTINGS_CATEGORIES: SettingsCategoryItem[] = [
  {
    id: "voice",
    label: "Voice",
    summary: "Audio devices and call behavior.",
  },
  {
    id: "profile",
    label: "Profile",
    summary: "Username, about, and avatar.",
  },
];

export const VOICE_SETTINGS_SUBMENU: VoiceSettingsSubmenuItem[] = [
  {
    id: "audio-devices",
    label: "Audio Devices",
    summary: "Select microphone and speaker devices.",
  },
];
