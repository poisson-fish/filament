import type {
  AttachmentId,
  AttachmentRecord,
  ChannelKindName,
  FriendRecord,
  FriendRequestList,
  GuildId,
  GuildRecord,
  GuildVisibility,
  GuildRoleRecord,
  PermissionName,
  ProfileRecord,
  RoleColorHex,
  RoleName,
  SearchResults,
  UserId,
  WorkspaceRoleId,
} from "../../../domain/chat";
import {
  channelKindFromInput,
  guildVisibilityFromInput,
  roleFromInput,
} from "../../../domain/chat";
import type {
  AudioDeviceOption,
  VoiceDevicePreferences,
} from "../../../lib/voice-device-settings";
import type { PanelHostProps } from "../components/panels/PanelHost";
import { SETTINGS_CATEGORIES, VOICE_SETTINGS_SUBMENU } from "../config/settings-menu";
import type {
  PublicDirectoryJoinStatus,
  SettingsCategory,
  VoiceSettingsSubmenu,
  WorkspaceSettingsSection,
} from "../types";
import type { DiagnosticsEventCounts } from "../state/diagnostics-event-counters";

export type PanelHostPropGroups = Pick<
  PanelHostProps,
  | "workspaceCreatePanelProps"
  | "channelCreatePanelProps"
  | "publicDirectoryPanelProps"
  | "settingsPanelProps"
  | "workspaceSettingsPanelProps"
  | "friendshipsPanelProps"
  | "searchPanelProps"
  | "attachmentsPanelProps"
  | "moderationPanelProps"
  | "roleManagementPanelProps"
  | "utilityPanelProps"
>;

export interface WorkspaceCreatePanelBuilderOptions {
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
}

export interface ChannelCreatePanelBuilderOptions {
  newChannelName: string;
  newChannelKind: ChannelKindName;
  isCreatingChannel: boolean;
  channelCreateError: string;
  onCreateChannelSubmit: (event: SubmitEvent) => Promise<void> | void;
  setNewChannelName: (value: string) => void;
  setNewChannelKind: (value: ChannelKindName) => void;
  onCancelChannelCreate: () => void;
}

export interface PublicDirectoryPanelBuilderOptions {
  publicGuildSearchQuery: string;
  isSearchingPublicGuilds: boolean;
  publicGuildSearchError: string;
  publicGuildDirectory: GuildRecord[];
  publicGuildJoinStatusByGuildId: Record<string, PublicDirectoryJoinStatus>;
  publicGuildJoinErrorByGuildId: Record<string, string>;
  onSubmitPublicGuildSearch: (event: SubmitEvent) => Promise<void> | void;
  onJoinGuildFromDirectory: (guildId: GuildId) => Promise<void> | void;
  setPublicGuildSearchQuery: (value: string) => void;
}

export interface SettingsPanelBuilderOptions {
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
}

export interface WorkspaceSettingsPanelBuilderOptions {
  hasActiveWorkspace: boolean;
  canManageWorkspaceSettings: boolean;
  canManageMemberRoles: boolean;
  workspaceSettingsSection: WorkspaceSettingsSection;
  workspaceName: string;
  workspaceVisibility: GuildVisibility;
  isSavingWorkspaceSettings: boolean;
  workspaceSettingsStatus: string;
  workspaceSettingsError: string;
  memberRoleStatus: string;
  memberRoleError: string;
  isMutatingMemberRoles: boolean;
  isLoadingMembers: boolean;
  memberListError: string;
  viewAsRoleSimulatorEnabled: boolean;
  viewAsRoleSimulatorRole: RoleName;
  members: Array<{
    userId: string;
    label: string;
    roleIds: WorkspaceRoleId[];
  }>;
  roles: GuildRoleRecord[];
  assignableRoleIds: WorkspaceRoleId[];
  setWorkspaceSettingsSection?: (value: WorkspaceSettingsSection) => void;
  setWorkspaceSettingsName: (value: string) => void;
  setWorkspaceSettingsVisibility: (value: GuildVisibility) => void;
  setViewAsRoleSimulatorEnabled: (value: boolean) => void;
  setViewAsRoleSimulatorRole: (value: RoleName) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
  onAssignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
}

export interface FriendshipsPanelBuilderOptions {
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
}

export interface SearchPanelBuilderOptions {
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
  resolveUserNameColor?: (userId: string) => string | null;
}

export interface AttachmentsPanelBuilderOptions {
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
}

export interface ModerationPanelBuilderOptions {
  moderationUserIdInput: string;
  moderationRoleInput: RoleName;
  overrideRoleInput: RoleName;
  overrideAllowCsv: string;
  overrideDenyCsv: string;
  channelOverrideEntities?: Array<{
    role: RoleName;
    label: string;
    hasExplicitOverride: boolean;
    allow: PermissionName[];
    deny: PermissionName[];
    updatedAtUnix: number | null;
  }>;
  channelOverrideEffectivePermissions: Record<RoleName, PermissionName[]>;
  isModerating: boolean;
  hasActiveWorkspace: boolean;
  hasActiveChannel: boolean;
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
  onOpenRoleManagementPanel: () => void;
}

export interface RoleManagementPanelBuilderOptions {
  hasActiveWorkspace: boolean;
  canManageWorkspaceRoles: boolean;
  canManageMemberRoles: boolean;
  roles: GuildRoleRecord[];
  isLoadingRoles: boolean;
  isMutatingRoles: boolean;
  roleManagementStatus: string;
  roleManagementError: string;
  defaultJoinRoleId?: WorkspaceRoleId | null;
  targetUserIdInput: string;
  setTargetUserIdInput: (value: string) => void;
  onRefreshRoles: () => Promise<void> | void;
  onCreateRole: (input: {
    name: string;
    permissions: PermissionName[];
    position?: number;
    colorHex?: RoleColorHex | null;
  }) => Promise<void> | void;
  onUpdateRole: (
    roleId: WorkspaceRoleId,
    input: {
      name?: string;
      permissions?: PermissionName[];
      colorHex?: RoleColorHex | null;
    },
  ) => Promise<void> | void;
  onDeleteRole: (roleId: WorkspaceRoleId) => Promise<void> | void;
  onReorderRoles: (roleIds: WorkspaceRoleId[]) => Promise<void> | void;
  onAssignRole: (targetUserIdInput: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignRole: (targetUserIdInput: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUpdateDefaultJoinRole?: (roleId: WorkspaceRoleId | null) => Promise<void> | void;
  onOpenModerationPanel: () => void;
}

export interface UtilityPanelBuilderOptions {
  echoInput: string;
  healthStatus: string;
  diagError: string;
  diagnosticsEventCounts: DiagnosticsEventCounts;
  showDiagnosticsCounters: boolean;
  isCheckingHealth: boolean;
  isEchoing: boolean;
  setEchoInput: (value: string) => void;
  onRunHealthCheck: () => Promise<void> | void;
  onRunEcho: (event: SubmitEvent) => Promise<void> | void;
}

export interface BuildPanelHostPropGroupsOptions {
  workspaceCreate: WorkspaceCreatePanelBuilderOptions;
  channelCreate: ChannelCreatePanelBuilderOptions;
  publicDirectory: PublicDirectoryPanelBuilderOptions;
  settings: SettingsPanelBuilderOptions;
  workspaceSettings: WorkspaceSettingsPanelBuilderOptions;
  friendships: FriendshipsPanelBuilderOptions;
  search: SearchPanelBuilderOptions;
  attachments: AttachmentsPanelBuilderOptions;
  moderation: ModerationPanelBuilderOptions;
  roleManagement: RoleManagementPanelBuilderOptions;
  utility: UtilityPanelBuilderOptions;
}

export function buildWorkspaceCreatePanelProps(
  options: WorkspaceCreatePanelBuilderOptions,
): PanelHostProps["workspaceCreatePanelProps"] {
  return {
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
  };
}

export function buildChannelCreatePanelProps(
  options: ChannelCreatePanelBuilderOptions,
): PanelHostProps["channelCreatePanelProps"] {
  return {
    newChannelName: options.newChannelName,
    newChannelKind: options.newChannelKind,
    isCreatingChannel: options.isCreatingChannel,
    channelCreateError: options.channelCreateError,
    onSubmit: options.onCreateChannelSubmit,
    onNewChannelNameInput: options.setNewChannelName,
    onNewChannelKindChange: (value) =>
      options.setNewChannelKind(channelKindFromInput(value)),
    onCancel: options.onCancelChannelCreate,
  };
}

export function buildPublicDirectoryPanelProps(
  options: PublicDirectoryPanelBuilderOptions,
): PanelHostProps["publicDirectoryPanelProps"] {
  return {
    searchQuery: options.publicGuildSearchQuery,
    isSearching: options.isSearchingPublicGuilds,
    searchError: options.publicGuildSearchError,
    guilds: options.publicGuildDirectory,
    joinStatusByGuildId: options.publicGuildJoinStatusByGuildId,
    joinErrorByGuildId: options.publicGuildJoinErrorByGuildId,
    onSubmitSearch: options.onSubmitPublicGuildSearch,
    onJoinGuild: options.onJoinGuildFromDirectory,
    onSearchInput: options.setPublicGuildSearchQuery,
  };
}

export function buildSettingsPanelProps(
  options: SettingsPanelBuilderOptions,
): PanelHostProps["settingsPanelProps"] {
  return {
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
  };
}

export function buildWorkspaceSettingsPanelProps(
  options: WorkspaceSettingsPanelBuilderOptions,
): PanelHostProps["workspaceSettingsPanelProps"] {
  return {
    hasActiveWorkspace: options.hasActiveWorkspace,
    canManageWorkspaceSettings: options.canManageWorkspaceSettings,
    canManageMemberRoles: options.canManageMemberRoles,
    activeSectionId: options.workspaceSettingsSection,
    workspaceName: options.workspaceName,
    workspaceVisibility: options.workspaceVisibility,
    isSavingWorkspaceSettings: options.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceSettingsError,
    memberRoleStatus: options.memberRoleStatus,
    memberRoleError: options.memberRoleError,
    isMutatingMemberRoles: options.isMutatingMemberRoles,
    isLoadingMembers: options.isLoadingMembers,
    memberListError: options.memberListError,
    viewAsRoleSimulatorEnabled: options.viewAsRoleSimulatorEnabled,
    viewAsRoleSimulatorRole: options.viewAsRoleSimulatorRole,
    members: options.members,
    roles: options.roles,
    assignableRoleIds: options.assignableRoleIds,
    setWorkspaceSettingsSection: options.setWorkspaceSettingsSection,
    onWorkspaceNameInput: options.setWorkspaceSettingsName,
    onWorkspaceVisibilityChange: options.setWorkspaceSettingsVisibility,
    onViewAsRoleSimulatorToggle: options.setViewAsRoleSimulatorEnabled,
    onViewAsRoleSimulatorRoleChange: (value) =>
      options.setViewAsRoleSimulatorRole(roleFromInput(value)),
    onSaveWorkspaceSettings: options.onSaveWorkspaceSettings,
    onAssignMemberRole: options.onAssignMemberRole,
    onUnassignMemberRole: options.onUnassignMemberRole,
  };
}

export function buildFriendshipsPanelProps(
  options: FriendshipsPanelBuilderOptions,
): PanelHostProps["friendshipsPanelProps"] {
  return {
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
  };
}

export function buildSearchPanelProps(
  options: SearchPanelBuilderOptions,
): PanelHostProps["searchPanelProps"] {
  return {
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
    resolveUserNameColor: options.resolveUserNameColor,
  };
}

export function buildAttachmentsPanelProps(
  options: AttachmentsPanelBuilderOptions,
): PanelHostProps["attachmentsPanelProps"] {
  return {
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
  };
}

export function buildModerationPanelProps(
  options: ModerationPanelBuilderOptions,
): PanelHostProps["moderationPanelProps"] {
  return {
    moderationUserIdInput: options.moderationUserIdInput,
    moderationRoleInput: options.moderationRoleInput,
    overrideRoleInput: options.overrideRoleInput,
    overrideAllowCsv: options.overrideAllowCsv,
    overrideDenyCsv: options.overrideDenyCsv,
    channelOverrideEntities: options.channelOverrideEntities ?? [],
    channelOverrideEffectivePermissions: options.channelOverrideEffectivePermissions,
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
    onOpenRoleManagementPanel: options.onOpenRoleManagementPanel,
  };
}

export function buildRoleManagementPanelProps(
  options: RoleManagementPanelBuilderOptions,
): PanelHostProps["roleManagementPanelProps"] {
  return {
    hasActiveWorkspace: options.hasActiveWorkspace,
    canManageWorkspaceRoles: options.canManageWorkspaceRoles,
    canManageMemberRoles: options.canManageMemberRoles,
    roles: options.roles,
    isLoadingRoles: options.isLoadingRoles,
    isMutatingRoles: options.isMutatingRoles,
    roleManagementStatus: options.roleManagementStatus,
    roleManagementError: options.roleManagementError,
    defaultJoinRoleId: options.defaultJoinRoleId,
    targetUserIdInput: options.targetUserIdInput,
    onTargetUserIdInput: options.setTargetUserIdInput,
    onRefreshRoles: options.onRefreshRoles,
    onCreateRole: options.onCreateRole,
    onUpdateRole: options.onUpdateRole,
    onDeleteRole: options.onDeleteRole,
    onReorderRoles: options.onReorderRoles,
    onAssignRole: options.onAssignRole,
    onUnassignRole: options.onUnassignRole,
    onUpdateDefaultJoinRole: options.onUpdateDefaultJoinRole,
    onOpenModerationPanel: options.onOpenModerationPanel,
  };
}

export function buildUtilityPanelProps(
  options: UtilityPanelBuilderOptions,
): PanelHostProps["utilityPanelProps"] {
  return {
    echoInput: options.echoInput,
    healthStatus: options.healthStatus,
    diagError: options.diagError,
    diagnosticsEventCounts: options.diagnosticsEventCounts,
    showDiagnosticsCounters: options.showDiagnosticsCounters,
    isCheckingHealth: options.isCheckingHealth,
    isEchoing: options.isEchoing,
    onEchoInput: options.setEchoInput,
    onRunHealthCheck: options.onRunHealthCheck,
    onRunEcho: options.onRunEcho,
  };
}

export function buildPanelHostPropGroups(
  options: BuildPanelHostPropGroupsOptions,
): PanelHostPropGroups {
  return {
    workspaceCreatePanelProps: buildWorkspaceCreatePanelProps(options.workspaceCreate),
    channelCreatePanelProps: buildChannelCreatePanelProps(options.channelCreate),
    publicDirectoryPanelProps: buildPublicDirectoryPanelProps(options.publicDirectory),
    settingsPanelProps: buildSettingsPanelProps(options.settings),
    workspaceSettingsPanelProps: buildWorkspaceSettingsPanelProps(options.workspaceSettings),
    friendshipsPanelProps: buildFriendshipsPanelProps(options.friendships),
    searchPanelProps: buildSearchPanelProps(options.search),
    attachmentsPanelProps: buildAttachmentsPanelProps(options.attachments),
    moderationPanelProps: buildModerationPanelProps(options.moderation),
    roleManagementPanelProps: buildRoleManagementPanelProps(options.roleManagement),
    utilityPanelProps: buildUtilityPanelProps(options.utility),
  };
}
