import type {
  AttachmentId,
  AttachmentRecord,
  ChannelKindName,
  FriendRecord,
  FriendRequestList,
  GuildRecord,
  GuildVisibility,
  ProfileRecord,
  RoleName,
  SearchResults,
  UserId,
} from "../../../domain/chat";
import {
  channelKindFromInput,
  guildVisibilityFromInput,
  roleFromInput,
} from "../../../domain/chat";
import { SETTINGS_CATEGORIES, VOICE_SETTINGS_SUBMENU } from "../config/settings-menu";
import type { PanelHostProps } from "../components/panels/PanelHost";
import type { SettingsCategory, VoiceSettingsSubmenu } from "../types";
import type {
  AudioDeviceOption,
  VoiceDevicePreferences,
} from "../../../lib/voice-device-settings";

export type PanelHostPropGroups = Pick<
  PanelHostProps,
  | "workspaceCreatePanelProps"
  | "channelCreatePanelProps"
  | "publicDirectoryPanelProps"
  | "settingsPanelProps"
  | "friendshipsPanelProps"
  | "searchPanelProps"
  | "attachmentsPanelProps"
  | "moderationPanelProps"
  | "utilityPanelProps"
>;

export interface BuildPanelHostPropGroupsOptions {
  createGuildName: string;
  createGuildVisibility: GuildVisibility;
  createChannelName: string;
  createChannelKind: ChannelKindName;
  isCreatingWorkspace: boolean;
  canDismissWorkspaceCreateForm: boolean;
  workspaceError: string;
  onCreateWorkspaceSubmit: (event: SubmitEvent) => Promise<void> | void;
  setCreateGuildName: (value: string) => void;
  setCreateGuildVisibility: (value: GuildVisibility) => void;
  setCreateChannelName: (value: string) => void;
  setCreateChannelKind: (value: ChannelKindName) => void;
  onCancelWorkspaceCreate: () => void;

  newChannelName: string;
  newChannelKind: ChannelKindName;
  isCreatingChannel: boolean;
  channelCreateError: string;
  onCreateChannelSubmit: (event: SubmitEvent) => Promise<void> | void;
  setNewChannelName: (value: string) => void;
  setNewChannelKind: (value: ChannelKindName) => void;
  onCancelChannelCreate: () => void;

  publicGuildSearchQuery: string;
  isSearchingPublicGuilds: boolean;
  publicGuildSearchError: string;
  publicGuildDirectory: GuildRecord[];
  onSubmitPublicGuildSearch: (event: SubmitEvent) => Promise<void> | void;
  setPublicGuildSearchQuery: (value: string) => void;

  activeSettingsCategory: SettingsCategory;
  activeVoiceSettingsSubmenu: VoiceSettingsSubmenu;
  voiceDevicePreferences: VoiceDevicePreferences;
  audioInputDevices: AudioDeviceOption[];
  audioOutputDevices: AudioDeviceOption[];
  isRefreshingAudioDevices: boolean;
  audioDevicesStatus: string;
  audioDevicesError: string;
  profile: ProfileRecord | null;
  profileDraftUsername: string;
  profileDraftAbout: string;
  profileAvatarUrl: string | null;
  selectedAvatarFilename: string;
  isSavingProfile: boolean;
  isUploadingProfileAvatar: boolean;
  profileSettingsStatus: string;
  profileSettingsError: string;
  onOpenSettingsCategory: (category: SettingsCategory) => void;
  onOpenVoiceSettingsSubmenu: (submenu: VoiceSettingsSubmenu) => void;
  onSetVoiceDevicePreference: (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ) => Promise<void> | void;
  onRefreshAudioDeviceInventory: () => Promise<void> | void;
  setProfileDraftUsername: (value: string) => void;
  setProfileDraftAbout: (value: string) => void;
  setSelectedProfileAvatarFile: (file: File | null) => void;
  onSaveProfileSettings: () => Promise<void> | void;
  onUploadProfileAvatar: () => Promise<void> | void;

  friendRecipientUserIdInput: string;
  friendRequests: FriendRequestList;
  friends: FriendRecord[];
  isRunningFriendAction: boolean;
  friendStatus: string;
  friendError: string;
  onSubmitFriendRequest: (event: SubmitEvent) => Promise<void> | void;
  setFriendRecipientUserIdInput: (value: string) => void;
  onAcceptIncomingFriendRequest: (requestId: string) => Promise<void> | void;
  onDismissFriendRequest: (requestId: string) => Promise<void> | void;
  onRemoveFriendship: (friendUserId: UserId) => Promise<void> | void;

  searchQuery: string;
  isSearching: boolean;
  hasActiveWorkspace: boolean;
  canManageSearchMaintenance: boolean;
  isRunningSearchOps: boolean;
  searchOpsStatus: string;
  searchError: string;
  searchResults: SearchResults | null;
  onSubmitSearch: (event: SubmitEvent) => Promise<void> | void;
  setSearchQuery: (value: string) => void;
  onRebuildSearch: () => Promise<void> | void;
  onReconcileSearch: () => Promise<void> | void;
  displayUserLabel: (userId: string) => string;

  attachmentFilename: string;
  activeAttachments: AttachmentRecord[];
  isUploadingAttachment: boolean;
  hasActiveChannel: boolean;
  attachmentStatus: string;
  attachmentError: string;
  downloadingAttachmentId: AttachmentId | null;
  deletingAttachmentId: AttachmentId | null;
  onSubmitUploadAttachment: (event: SubmitEvent) => Promise<void> | void;
  setSelectedAttachment: (file: File | null) => void;
  setAttachmentFilename: (value: string) => void;
  onDownloadAttachment: (record: AttachmentRecord) => Promise<void> | void;
  onRemoveAttachment: (record: AttachmentRecord) => Promise<void> | void;

  moderationUserIdInput: string;
  moderationRoleInput: RoleName;
  overrideRoleInput: RoleName;
  overrideAllowCsv: string;
  overrideDenyCsv: string;
  isModerating: boolean;
  canManageRoles: boolean;
  canBanMembers: boolean;
  canManageChannelOverrides: boolean;
  moderationStatus: string;
  moderationError: string;
  setModerationUserIdInput: (value: string) => void;
  setModerationRoleInput: (value: RoleName) => void;
  onRunMemberAction: (action: "add" | "role" | "kick" | "ban") => Promise<void> | void;
  setOverrideRoleInput: (value: RoleName) => void;
  setOverrideAllowCsv: (value: string) => void;
  setOverrideDenyCsv: (value: string) => void;
  onApplyOverride: (event: SubmitEvent) => Promise<void> | void;

  echoInput: string;
  healthStatus: string;
  diagError: string;
  isCheckingHealth: boolean;
  isEchoing: boolean;
  setEchoInput: (value: string) => void;
  onRunHealthCheck: () => Promise<void> | void;
  onRunEcho: (event: SubmitEvent) => Promise<void> | void;
}

export function buildPanelHostPropGroups(
  options: BuildPanelHostPropGroupsOptions,
): PanelHostPropGroups {
  return {
    workspaceCreatePanelProps: {
      createGuildName: options.createGuildName,
      createGuildVisibility: options.createGuildVisibility,
      createChannelName: options.createChannelName,
      createChannelKind: options.createChannelKind,
      isCreatingWorkspace: options.isCreatingWorkspace,
      canDismissWorkspaceCreateForm: options.canDismissWorkspaceCreateForm,
      workspaceError: options.workspaceError,
      onSubmit: options.onCreateWorkspaceSubmit,
      onCreateGuildNameInput: options.setCreateGuildName,
      onCreateGuildVisibilityChange: (value) =>
        options.setCreateGuildVisibility(guildVisibilityFromInput(value)),
      onCreateChannelNameInput: options.setCreateChannelName,
      onCreateChannelKindChange: (value) =>
        options.setCreateChannelKind(channelKindFromInput(value)),
      onCancel: options.onCancelWorkspaceCreate,
    },
    channelCreatePanelProps: {
      newChannelName: options.newChannelName,
      newChannelKind: options.newChannelKind,
      isCreatingChannel: options.isCreatingChannel,
      channelCreateError: options.channelCreateError,
      onSubmit: options.onCreateChannelSubmit,
      onNewChannelNameInput: options.setNewChannelName,
      onNewChannelKindChange: (value) =>
        options.setNewChannelKind(channelKindFromInput(value)),
      onCancel: options.onCancelChannelCreate,
    },
    publicDirectoryPanelProps: {
      searchQuery: options.publicGuildSearchQuery,
      isSearching: options.isSearchingPublicGuilds,
      searchError: options.publicGuildSearchError,
      guilds: options.publicGuildDirectory,
      onSubmitSearch: options.onSubmitPublicGuildSearch,
      onSearchInput: options.setPublicGuildSearchQuery,
    },
    settingsPanelProps: {
      settingsCategories: SETTINGS_CATEGORIES,
      voiceSettingsSubmenu: VOICE_SETTINGS_SUBMENU,
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
      profileAvatarUrl: options.profileAvatarUrl,
      selectedAvatarFilename: options.selectedAvatarFilename,
      isSavingProfile: options.isSavingProfile,
      isUploadingProfileAvatar: options.isUploadingProfileAvatar,
      profileStatus: options.profileSettingsStatus,
      profileError: options.profileSettingsError,
      onOpenSettingsCategory: options.onOpenSettingsCategory,
      onOpenVoiceSettingsSubmenu: options.onOpenVoiceSettingsSubmenu,
      onSetVoiceDevicePreference: options.onSetVoiceDevicePreference,
      onRefreshAudioDeviceInventory: options.onRefreshAudioDeviceInventory,
      onProfileUsernameInput: options.setProfileDraftUsername,
      onProfileAboutInput: options.setProfileDraftAbout,
      onSelectProfileAvatarFile: options.setSelectedProfileAvatarFile,
      onSaveProfile: options.onSaveProfileSettings,
      onUploadProfileAvatar: options.onUploadProfileAvatar,
    },
    friendshipsPanelProps: {
      friendRecipientUserIdInput: options.friendRecipientUserIdInput,
      friendRequests: options.friendRequests,
      friends: options.friends,
      isRunningFriendAction: options.isRunningFriendAction,
      friendStatus: options.friendStatus,
      friendError: options.friendError,
      onSubmitFriendRequest: options.onSubmitFriendRequest,
      onFriendRecipientInput: options.setFriendRecipientUserIdInput,
      onAcceptIncomingFriendRequest: options.onAcceptIncomingFriendRequest,
      onDismissFriendRequest: options.onDismissFriendRequest,
      onRemoveFriendship: options.onRemoveFriendship,
    },
    searchPanelProps: {
      searchQuery: options.searchQuery,
      isSearching: options.isSearching,
      hasActiveWorkspace: options.hasActiveWorkspace,
      canManageSearchMaintenance: options.canManageSearchMaintenance,
      isRunningSearchOps: options.isRunningSearchOps,
      searchOpsStatus: options.searchOpsStatus,
      searchError: options.searchError,
      searchResults: options.searchResults,
      onSubmitSearch: options.onSubmitSearch,
      onSearchQueryInput: options.setSearchQuery,
      onRebuildSearch: options.onRebuildSearch,
      onReconcileSearch: options.onReconcileSearch,
      displayUserLabel: options.displayUserLabel,
    },
    attachmentsPanelProps: {
      attachmentFilename: options.attachmentFilename,
      activeAttachments: options.activeAttachments,
      isUploadingAttachment: options.isUploadingAttachment,
      hasActiveChannel: options.hasActiveChannel,
      attachmentStatus: options.attachmentStatus,
      attachmentError: options.attachmentError,
      downloadingAttachmentId: options.downloadingAttachmentId,
      deletingAttachmentId: options.deletingAttachmentId,
      onSubmitUpload: options.onSubmitUploadAttachment,
      onAttachmentFileInput: (file) => {
        options.setSelectedAttachment(file);
        options.setAttachmentFilename(file?.name ?? "");
      },
      onAttachmentFilenameInput: options.setAttachmentFilename,
      onDownloadAttachment: options.onDownloadAttachment,
      onRemoveAttachment: options.onRemoveAttachment,
    },
    moderationPanelProps: {
      moderationUserIdInput: options.moderationUserIdInput,
      moderationRoleInput: options.moderationRoleInput,
      overrideRoleInput: options.overrideRoleInput,
      overrideAllowCsv: options.overrideAllowCsv,
      overrideDenyCsv: options.overrideDenyCsv,
      isModerating: options.isModerating,
      hasActiveWorkspace: options.hasActiveWorkspace,
      hasActiveChannel: options.hasActiveChannel,
      canManageRoles: options.canManageRoles,
      canBanMembers: options.canBanMembers,
      canManageChannelOverrides: options.canManageChannelOverrides,
      moderationStatus: options.moderationStatus,
      moderationError: options.moderationError,
      onModerationUserIdInput: options.setModerationUserIdInput,
      onModerationRoleChange: (value) =>
        options.setModerationRoleInput(roleFromInput(value)),
      onRunMemberAction: options.onRunMemberAction,
      onOverrideRoleChange: (value) =>
        options.setOverrideRoleInput(roleFromInput(value)),
      onOverrideAllowInput: options.setOverrideAllowCsv,
      onOverrideDenyInput: options.setOverrideDenyCsv,
      onApplyOverride: options.onApplyOverride,
    },
    utilityPanelProps: {
      echoInput: options.echoInput,
      healthStatus: options.healthStatus,
      diagError: options.diagError,
      isCheckingHealth: options.isCheckingHealth,
      isEchoing: options.isEchoing,
      onEchoInput: options.setEchoInput,
      onRunHealthCheck: options.onRunHealthCheck,
      onRunEcho: options.onRunEcho,
    },
  };
}
