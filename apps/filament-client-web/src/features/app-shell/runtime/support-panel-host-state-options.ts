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
import type { WorkspaceRoleId } from "../../../domain/chat";
import { MAX_WORKSPACE_SETTINGS_MEMBERS } from "../config/workspace-member-list";

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
  displayUserLabel: (userId: string) => string;
  workspaceMembersByGuildId: () => Record<string, string[]>;
  isLoadingWorkspaceMembers: () => boolean;
  workspaceMembersError: () => string;
  isDevelopmentMode: boolean;
}

export function createSupportPanelHostStateOptions(
  options: SupportPanelHostStateOptions,
): SupportPanelPropGroupsStateOptions {
  const resolveWorkspaceSettingsMemberRows = (): Array<{
    userId: string;
    label: string;
    roleIds: WorkspaceRoleId[];
  }> => {
    const guildId = options.workspaceChannelState.activeGuildId();
    if (!guildId) {
      return [];
    }
    const assignmentsByUser =
      options.workspaceChannelState.workspaceUserRolesByGuildId()[guildId] ?? {};
    const knownMemberIds = new Set<string>();
    const roster = options.workspaceMembersByGuildId()[guildId] ?? [];
    for (const userId of roster) {
      knownMemberIds.add(userId);
      if (knownMemberIds.size >= MAX_WORKSPACE_SETTINGS_MEMBERS) {
        break;
      }
    }
    const actorId = options.profileController.profile()?.userId;
    if (actorId) {
      knownMemberIds.add(actorId);
    }
    for (const userId of options.profileState.onlineMembers()) {
      knownMemberIds.add(userId);
      if (knownMemberIds.size >= MAX_WORKSPACE_SETTINGS_MEMBERS) {
        break;
      }
    }
    if (knownMemberIds.size < MAX_WORKSPACE_SETTINGS_MEMBERS) {
      for (const userId of Object.keys(assignmentsByUser)) {
        knownMemberIds.add(userId);
        if (knownMemberIds.size >= MAX_WORKSPACE_SETTINGS_MEMBERS) {
          break;
        }
      }
    }

    const rows: Array<{
      userId: string;
      label: string;
      roleIds: WorkspaceRoleId[];
    }> = [];
    for (const userId of knownMemberIds) {
      rows.push({
        userId,
        label: options.displayUserLabel(userId),
        roleIds: assignmentsByUser[userId] ?? [],
      });
    }
    rows.sort((left, right) => left.userId.localeCompare(right.userId));
    return rows;
  };

  const resolveAssignableRoleIds = (): WorkspaceRoleId[] => {
    const guildId = options.workspaceChannelState.activeGuildId();
    if (!guildId) {
      return [];
    }
    const allRoles = options.roleManagementActions.roles();
    const actorId = options.profileController.profile()?.userId;
    if (!actorId) {
      return [];
    }
    const actorRoleIds =
      options.workspaceChannelState.workspaceUserRolesByGuildId()[guildId]?.[actorId] ?? [];
    if (actorRoleIds.length === 0) {
      return [];
    }
    const roleById = new Map(allRoles.map((role) => [role.roleId, role]));
    const actorHighestPosition = actorRoleIds.reduce((highest, roleId) => {
      const role = roleById.get(roleId);
      if (!role) {
        return highest;
      }
      return Math.max(highest, role.position);
    }, Number.NEGATIVE_INFINITY);
    return allRoles
      .filter((role) => !role.isSystem && role.position < actorHighestPosition)
      .map((role) => role.roleId);
  };

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
    workspaceSettingsSection: options.overlayState.activeWorkspaceSettingsSection,
    workspaceName: options.workspaceChannelState.workspaceSettingsName,
    workspaceVisibility: options.workspaceChannelState.workspaceSettingsVisibility,
    isSavingWorkspaceSettings: options.workspaceChannelState.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceChannelState.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceChannelState.workspaceSettingsError,
    memberRoleStatus: options.roleManagementActions.roleManagementStatus,
    memberRoleError: options.roleManagementActions.roleManagementError,
    isMutatingMemberRoles: options.roleManagementActions.isMutatingRoles,
    viewAsRoleSimulatorEnabled:
      options.workspaceChannelState.viewAsRoleSimulatorEnabled,
    viewAsRoleSimulatorRole: options.workspaceChannelState.viewAsRoleSimulatorRole,
    members: resolveWorkspaceSettingsMemberRows,
    isLoadingWorkspaceMembers: options.isLoadingWorkspaceMembers,
    workspaceMembersError: options.workspaceMembersError,
    assignableRoleIds: resolveAssignableRoleIds,
    setWorkspaceSettingsSection:
      options.overlayState.setActiveWorkspaceSettingsSection,
    setWorkspaceSettingsName: options.workspaceChannelState.setWorkspaceSettingsName,
    setWorkspaceSettingsVisibility:
      options.workspaceChannelState.setWorkspaceSettingsVisibility,
    setViewAsRoleSimulatorEnabled:
      options.workspaceChannelState.setViewAsRoleSimulatorEnabled,
    setViewAsRoleSimulatorRole: options.workspaceChannelState.setViewAsRoleSimulatorRole,
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
    onAssignMemberRole: (userId, roleId) =>
      options.roleManagementActions.assignRoleToMember(userId, roleId),
    onUnassignMemberRole: (userId, roleId) =>
      options.roleManagementActions.unassignRoleFromMember(userId, roleId),
    echoInput: options.diagnosticsState.echoInput,
    healthStatus: options.diagnosticsState.healthStatus,
    diagError: options.diagnosticsState.diagError,
    diagnosticsEventCounts: options.diagnosticsState.diagnosticsEventCounts,
    showDiagnosticsCounters: options.isDevelopmentMode,
    isCheckingHealth: options.diagnosticsState.isCheckingHealth,
    isEchoing: options.diagnosticsState.isEchoing,
    setEchoInput: options.diagnosticsState.setEchoInput,
    onRunHealthCheck: options.sessionDiagnostics.runHealthCheck,
    onRunEcho: options.sessionDiagnostics.runEcho,
  };
}
