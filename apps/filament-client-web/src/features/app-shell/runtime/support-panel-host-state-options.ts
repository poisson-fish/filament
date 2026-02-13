import type { createProfileController } from "../controllers/profile-controller";
import type { createPublicDirectoryController } from "../controllers/public-directory-controller";
import type { createRoleManagementController } from "../controllers/role-management-controller";
import type { CreateAppShellSelectorsResult } from "../selectors/create-app-shell-selectors";
import type { createDiagnosticsState } from "../state/diagnostics-state";
import type { createOverlayState } from "../state/overlay-state";
import type { createProfileState } from "../state/profile-state";
import type { createVoiceState } from "../state/voice-state";
import type { createWorkspaceState } from "../state/workspace-state";
import type { createSessionDiagnosticsActions } from "./session-diagnostics-actions";
import type { SupportPanelPropGroupsStateOptions } from "./support-panel-prop-groups-options";

export interface SupportPanelHostStateOptions {
  discoveryState: ReturnType<typeof createWorkspaceState>["discovery"];
  overlayState: ReturnType<typeof createOverlayState>;
  voiceState: ReturnType<typeof createVoiceState>;
  profileState: ReturnType<typeof createProfileState>;
  workspaceChannelState: ReturnType<typeof createWorkspaceState>["workspaceChannel"];
  diagnosticsState: ReturnType<typeof createDiagnosticsState>;
  selectors: CreateAppShellSelectorsResult;
  publicDirectoryActions: ReturnType<typeof createPublicDirectoryController>;
  profileController: ReturnType<typeof createProfileController>;
  roleManagementActions: ReturnType<typeof createRoleManagementController>;
  sessionDiagnostics: ReturnType<typeof createSessionDiagnosticsActions>;
  openSettingsCategory: (category: "profile" | "voice") => void;
  setVoiceDevicePreference: (
    kind: "audioinput" | "audiooutput",
    value: string,
  ) => void;
  refreshAudioDeviceInventory: (force: boolean) => Promise<void>;
  saveWorkspaceSettings: () => Promise<void>;
  openOverlayPanel: (panel: "moderation") => void;
}

export function createSupportPanelHostStateOptions(
  options: SupportPanelHostStateOptions,
): SupportPanelPropGroupsStateOptions {
  return {
    publicGuildSearchQuery: options.discoveryState.publicGuildSearchQuery,
    isSearchingPublicGuilds: options.discoveryState.isSearchingPublicGuilds,
    publicGuildSearchError: options.discoveryState.publicGuildSearchError,
    publicGuildDirectory: options.discoveryState.publicGuildDirectory(),
    publicGuildJoinStatusByGuildId:
      options.discoveryState.publicGuildJoinStatusByGuildId(),
    publicGuildJoinErrorByGuildId:
      options.discoveryState.publicGuildJoinErrorByGuildId(),
    onSubmitPublicGuildSearch: options.publicDirectoryActions.runPublicGuildSearch,
    onJoinGuildFromDirectory: options.publicDirectoryActions.joinGuildFromDirectory,
    setPublicGuildSearchQuery: options.discoveryState.setPublicGuildSearchQuery,
    activeSettingsCategory: options.overlayState.activeSettingsCategory,
    activeVoiceSettingsSubmenu: options.overlayState.activeVoiceSettingsSubmenu,
    voiceDevicePreferences: options.voiceState.voiceDevicePreferences,
    audioInputDevices: options.voiceState.audioInputDevices,
    audioOutputDevices: options.voiceState.audioOutputDevices,
    isRefreshingAudioDevices: options.voiceState.isRefreshingAudioDevices,
    audioDevicesStatus: options.voiceState.audioDevicesStatus,
    audioDevicesError: options.voiceState.audioDevicesError,
    profile: () => options.profileController.profile() ?? null,
    profileDraftUsername: options.profileState.profileDraftUsername,
    profileDraftAbout: options.profileState.profileDraftAbout,
    selectedAvatarFilename: () =>
      options.profileState.selectedProfileAvatarFile()?.name ?? "",
    isSavingProfile: options.profileState.isSavingProfile,
    isUploadingProfileAvatar: options.profileState.isUploadingProfileAvatar,
    profileSettingsStatus: options.profileState.profileSettingsStatus,
    profileSettingsError: options.profileState.profileSettingsError,
    onOpenSettingsCategory: options.openSettingsCategory,
    onOpenVoiceSettingsSubmenu: options.overlayState.setActiveVoiceSettingsSubmenu,
    onSetVoiceDevicePreference: (kind, value) =>
      options.setVoiceDevicePreference(kind, value),
    onRefreshAudioDeviceInventory: () => options.refreshAudioDeviceInventory(true),
    setProfileDraftUsername: options.profileState.setProfileDraftUsername,
    setProfileDraftAbout: options.profileState.setProfileDraftAbout,
    setSelectedProfileAvatarFile: options.profileState.setSelectedProfileAvatarFile,
    onSaveProfileSettings: options.profileController.saveProfileSettings,
    onUploadProfileAvatar: options.profileController.uploadProfileAvatar,
    avatarUrlForUser: options.profileController.avatarUrlForUser,
    hasActiveWorkspace: () => Boolean(options.selectors.activeWorkspace()),
    canManageWorkspaceSettings: options.selectors.canManageRoles,
    workspaceName: options.workspaceChannelState.workspaceSettingsName,
    workspaceVisibility: options.workspaceChannelState.workspaceSettingsVisibility,
    isSavingWorkspaceSettings: options.workspaceChannelState.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceChannelState.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceChannelState.workspaceSettingsError,
    setWorkspaceSettingsName: options.workspaceChannelState.setWorkspaceSettingsName,
    setWorkspaceSettingsVisibility:
      options.workspaceChannelState.setWorkspaceSettingsVisibility,
    setWorkspaceSettingsStatus:
      options.workspaceChannelState.setWorkspaceSettingsStatus,
    setWorkspaceSettingsError: options.workspaceChannelState.setWorkspaceSettingsError,
    onSaveWorkspaceSettings: options.saveWorkspaceSettings,
    canManageWorkspaceRoles: options.selectors.canManageWorkspaceRoles,
    canManageMemberRoles: options.selectors.canManageMemberRoles,
    roles: options.roleManagementActions.roles,
    isLoadingRoles: options.roleManagementActions.isLoadingRoles,
    isMutatingRoles: options.roleManagementActions.isMutatingRoles,
    roleManagementStatus: options.roleManagementActions.roleManagementStatus,
    roleManagementError: options.roleManagementActions.roleManagementError,
    targetUserIdInput: options.diagnosticsState.moderationUserIdInput,
    setTargetUserIdInput: options.diagnosticsState.setModerationUserIdInput,
    onRefreshRoles: options.roleManagementActions.refreshRoles,
    onCreateRole: options.roleManagementActions.createRole,
    onUpdateRole: options.roleManagementActions.updateRole,
    onDeleteRole: options.roleManagementActions.deleteRole,
    onReorderRoles: options.roleManagementActions.reorderRoles,
    onAssignRole: options.roleManagementActions.assignRoleToMember,
    onUnassignRole: options.roleManagementActions.unassignRoleFromMember,
    onOpenModerationPanel: () => options.openOverlayPanel("moderation"),
    echoInput: options.diagnosticsState.echoInput,
    healthStatus: options.diagnosticsState.healthStatus,
    diagError: options.diagnosticsState.diagError,
    isCheckingHealth: options.diagnosticsState.isCheckingHealth,
    isEchoing: options.diagnosticsState.isEchoing,
    setEchoInput: options.diagnosticsState.setEchoInput,
    onRunHealthCheck: options.sessionDiagnostics.runHealthCheck,
    onRunEcho: options.sessionDiagnostics.runEcho,
  };
}
