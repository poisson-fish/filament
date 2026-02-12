import type { MediaPublishSource, ReactionEmoji } from "../../domain/chat";

export interface VoiceRosterEntry {
  identity: string;
  isLocal: boolean;
  isSpeaking: boolean;
  hasCamera: boolean;
  hasScreenShare: boolean;
}

export interface VoiceSessionCapabilities {
  canSubscribe: boolean;
  publishSources: MediaPublishSource[];
}

export interface ReactionPickerOption {
  emoji: ReactionEmoji;
  label: string;
  iconUrl: string;
}

export interface ReactionPickerOverlayPosition {
  top: number;
  left: number;
}

export type OverlayPanel =
  | "workspace-create"
  | "channel-create"
  | "settings"
  | "public-directory"
  | "friendships"
  | "search"
  | "attachments"
  | "moderation"
  | "utility";

export type SettingsCategory = "voice" | "profile";

export type VoiceSettingsSubmenu = "audio-devices";

export interface SettingsCategoryItem {
  id: SettingsCategory;
  label: string;
  summary: string;
}

export interface VoiceSettingsSubmenuItem {
  id: VoiceSettingsSubmenu;
  label: string;
  summary: string;
}

export type PublicDirectoryJoinStatus =
  | "idle"
  | "joining"
  | "joined"
  | "banned"
  | "join_failed";
