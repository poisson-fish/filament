import type { GuildVisibility, WorkspaceRoleId } from "../../../domain/chat";
import type { SupportPanelPropGroupsOptions } from "./support-panel-prop-groups";

export interface SupportPanelPropGroupsStateOptions {
  publicGuildSearchQuery: () => string;
  isSearchingPublicGuilds: () => boolean;
  publicGuildSearchError: () => string;
  publicGuildDirectory:
    SupportPanelPropGroupsOptions["publicDirectory"]["publicGuildDirectory"];
  publicGuildJoinStatusByGuildId:
    SupportPanelPropGroupsOptions["publicDirectory"]["publicGuildJoinStatusByGuildId"];
  publicGuildJoinErrorByGuildId:
    SupportPanelPropGroupsOptions["publicDirectory"]["publicGuildJoinErrorByGuildId"];
  onSubmitPublicGuildSearch:
    SupportPanelPropGroupsOptions["publicDirectory"]["onSubmitPublicGuildSearch"];
  onJoinGuildFromDirectory:
    SupportPanelPropGroupsOptions["publicDirectory"]["onJoinGuildFromDirectory"];
  setPublicGuildSearchQuery:
    SupportPanelPropGroupsOptions["publicDirectory"]["setPublicGuildSearchQuery"];
  activeSettingsCategory:
    () => SupportPanelPropGroupsOptions["settings"]["activeSettingsCategory"];
  activeVoiceSettingsSubmenu:
    () => SupportPanelPropGroupsOptions["settings"]["activeVoiceSettingsSubmenu"];
  voiceDevicePreferences:
    () => SupportPanelPropGroupsOptions["settings"]["voiceDevicePreferences"];
  audioInputDevices:
    () => SupportPanelPropGroupsOptions["settings"]["audioInputDevices"];
  audioOutputDevices:
    () => SupportPanelPropGroupsOptions["settings"]["audioOutputDevices"];
  isRefreshingAudioDevices: () => boolean;
  audioDevicesStatus: () => string;
  audioDevicesError: () => string;
  profile: () => SupportPanelPropGroupsOptions["settings"]["profile"];
  profileDraftUsername: () => string;
  profileDraftAbout: () => string;
  selectedAvatarFilename: () => string;
  isSavingProfile: () => boolean;
  isUploadingProfileAvatar: () => boolean;
  profileSettingsStatus: () => string;
  profileSettingsError: () => string;
  onOpenSettingsCategory:
    SupportPanelPropGroupsOptions["settings"]["onOpenSettingsCategory"];
  onOpenVoiceSettingsSubmenu:
    SupportPanelPropGroupsOptions["settings"]["onOpenVoiceSettingsSubmenu"];
  onSetVoiceDevicePreference:
    SupportPanelPropGroupsOptions["settings"]["onSetVoiceDevicePreference"];
  onRefreshAudioDeviceInventory:
    SupportPanelPropGroupsOptions["settings"]["onRefreshAudioDeviceInventory"];
  setProfileDraftUsername:
    SupportPanelPropGroupsOptions["settings"]["setProfileDraftUsername"];
  setProfileDraftAbout:
    SupportPanelPropGroupsOptions["settings"]["setProfileDraftAbout"];
  setSelectedProfileAvatarFile:
    SupportPanelPropGroupsOptions["settings"]["setSelectedProfileAvatarFile"];
  onSaveProfileSettings:
    SupportPanelPropGroupsOptions["settings"]["onSaveProfileSettings"];
  onUploadProfileAvatar:
    SupportPanelPropGroupsOptions["settings"]["onUploadProfileAvatar"];
  avatarUrlForUser:
    SupportPanelPropGroupsOptions["settings"]["avatarUrlForUser"];
  hasActiveWorkspace: () => boolean;
  canManageWorkspaceSettings: () => boolean;
  workspaceSettingsSection:
    () => SupportPanelPropGroupsOptions["workspaceSettings"]["workspaceSettingsSection"];
  workspaceName: () => string;
  workspaceVisibility: () => GuildVisibility;
  isSavingWorkspaceSettings: () => boolean;
  workspaceSettingsStatus: () => string;
  workspaceSettingsError: () => string;
  memberRoleStatus: () => string;
  memberRoleError: () => string;
  isMutatingMemberRoles: () => boolean;
  viewAsRoleSimulatorEnabled: () => boolean;
  viewAsRoleSimulatorRole:
    () => SupportPanelPropGroupsOptions["workspaceSettings"]["viewAsRoleSimulatorRole"];
  members: () => Array<{
    userId: string;
    label: string;
    roleIds: WorkspaceRoleId[];
  }>;
  assignableRoleIds: () => SupportPanelPropGroupsOptions["workspaceSettings"]["assignableRoleIds"];
  setWorkspaceSettingsName:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setWorkspaceSettingsName"];
  setWorkspaceSettingsVisibility:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setWorkspaceSettingsVisibility"];
  setViewAsRoleSimulatorEnabled:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setViewAsRoleSimulatorEnabled"];
  setViewAsRoleSimulatorRole:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setViewAsRoleSimulatorRole"];
  setWorkspaceSettingsStatus:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setWorkspaceSettingsStatus"];
  setWorkspaceSettingsError:
    SupportPanelPropGroupsOptions["workspaceSettings"]["setWorkspaceSettingsError"];
  onSaveWorkspaceSettings:
    SupportPanelPropGroupsOptions["workspaceSettings"]["onSaveWorkspaceSettings"];
  canManageWorkspaceRoles: () => boolean;
  canManageMemberRoles: () => boolean;
  roles: () => SupportPanelPropGroupsOptions["roleManagement"]["roles"];
  isLoadingRoles: () => boolean;
  isMutatingRoles: () => boolean;
  roleManagementStatus: () => string;
  roleManagementError: () => string;
  targetUserIdInput: () => string;
  setTargetUserIdInput:
    SupportPanelPropGroupsOptions["roleManagement"]["setTargetUserIdInput"];
  onRefreshRoles:
    SupportPanelPropGroupsOptions["roleManagement"]["onRefreshRoles"];
  onCreateRole:
    SupportPanelPropGroupsOptions["roleManagement"]["onCreateRole"];
  onUpdateRole:
    SupportPanelPropGroupsOptions["roleManagement"]["onUpdateRole"];
  onDeleteRole:
    SupportPanelPropGroupsOptions["roleManagement"]["onDeleteRole"];
  onReorderRoles:
    SupportPanelPropGroupsOptions["roleManagement"]["onReorderRoles"];
  onAssignRole:
    SupportPanelPropGroupsOptions["roleManagement"]["onAssignRole"];
  onUnassignRole:
    SupportPanelPropGroupsOptions["roleManagement"]["onUnassignRole"];
  onOpenModerationPanel:
    SupportPanelPropGroupsOptions["roleManagement"]["onOpenModerationPanel"];
  onAssignMemberRole:
    SupportPanelPropGroupsOptions["workspaceSettings"]["onAssignMemberRole"];
  onUnassignMemberRole:
    SupportPanelPropGroupsOptions["workspaceSettings"]["onUnassignMemberRole"];
  echoInput: () => string;
  healthStatus: () => string;
  diagError: () => string;
  diagnosticsEventCounts: () => SupportPanelPropGroupsOptions["utility"]["diagnosticsEventCounts"];
  showDiagnosticsCounters: boolean;
  isCheckingHealth: () => boolean;
  isEchoing: () => boolean;
  setEchoInput: SupportPanelPropGroupsOptions["utility"]["setEchoInput"];
  onRunHealthCheck: SupportPanelPropGroupsOptions["utility"]["onRunHealthCheck"];
  onRunEcho: SupportPanelPropGroupsOptions["utility"]["onRunEcho"];
}

export function createSupportPanelPropGroupsOptions(
  options: SupportPanelPropGroupsStateOptions,
): SupportPanelPropGroupsOptions {
  return {
    publicDirectory: {
      publicGuildSearchQuery: options.publicGuildSearchQuery(),
      isSearchingPublicGuilds: options.isSearchingPublicGuilds(),
      publicGuildSearchError: options.publicGuildSearchError(),
      publicGuildDirectory: options.publicGuildDirectory,
      publicGuildJoinStatusByGuildId: options.publicGuildJoinStatusByGuildId,
      publicGuildJoinErrorByGuildId: options.publicGuildJoinErrorByGuildId,
      onSubmitPublicGuildSearch: options.onSubmitPublicGuildSearch,
      onJoinGuildFromDirectory: options.onJoinGuildFromDirectory,
      setPublicGuildSearchQuery: options.setPublicGuildSearchQuery,
    },
    settings: {
      activeSettingsCategory: options.activeSettingsCategory(),
      activeVoiceSettingsSubmenu: options.activeVoiceSettingsSubmenu(),
      voiceDevicePreferences: options.voiceDevicePreferences(),
      audioInputDevices: options.audioInputDevices(),
      audioOutputDevices: options.audioOutputDevices(),
      isRefreshingAudioDevices: options.isRefreshingAudioDevices(),
      audioDevicesStatus: options.audioDevicesStatus(),
      audioDevicesError: options.audioDevicesError(),
      profile: options.profile(),
      profileDraftUsername: options.profileDraftUsername(),
      profileDraftAbout: options.profileDraftAbout(),
      selectedAvatarFilename: options.selectedAvatarFilename(),
      isSavingProfile: options.isSavingProfile(),
      isUploadingProfileAvatar: options.isUploadingProfileAvatar(),
      profileSettingsStatus: options.profileSettingsStatus(),
      profileSettingsError: options.profileSettingsError(),
      onOpenSettingsCategory: options.onOpenSettingsCategory,
      onOpenVoiceSettingsSubmenu: options.onOpenVoiceSettingsSubmenu,
      onSetVoiceDevicePreference: options.onSetVoiceDevicePreference,
      onRefreshAudioDeviceInventory: options.onRefreshAudioDeviceInventory,
      setProfileDraftUsername: options.setProfileDraftUsername,
      setProfileDraftAbout: options.setProfileDraftAbout,
      setSelectedProfileAvatarFile: options.setSelectedProfileAvatarFile,
      onSaveProfileSettings: options.onSaveProfileSettings,
      onUploadProfileAvatar: options.onUploadProfileAvatar,
      avatarUrlForUser: options.avatarUrlForUser,
    },
    workspaceSettings: {
      hasActiveWorkspace: options.hasActiveWorkspace(),
      canManageWorkspaceSettings: options.canManageWorkspaceSettings(),
      workspaceSettingsSection: options.workspaceSettingsSection(),
      canManageMemberRoles: options.canManageMemberRoles(),
      workspaceName: options.workspaceName(),
      workspaceVisibility: options.workspaceVisibility(),
      isSavingWorkspaceSettings: options.isSavingWorkspaceSettings(),
      workspaceSettingsStatus: options.workspaceSettingsStatus(),
      workspaceSettingsError: options.workspaceSettingsError(),
      memberRoleStatus: options.memberRoleStatus(),
      memberRoleError: options.memberRoleError(),
      isMutatingMemberRoles: options.isMutatingMemberRoles(),
      viewAsRoleSimulatorEnabled: options.viewAsRoleSimulatorEnabled(),
      viewAsRoleSimulatorRole: options.viewAsRoleSimulatorRole(),
      members: options.members(),
      roles: options.roles(),
      assignableRoleIds: options.assignableRoleIds(),
      setWorkspaceSettingsName: options.setWorkspaceSettingsName,
      setWorkspaceSettingsVisibility: options.setWorkspaceSettingsVisibility,
      setViewAsRoleSimulatorEnabled: options.setViewAsRoleSimulatorEnabled,
      setViewAsRoleSimulatorRole: options.setViewAsRoleSimulatorRole,
      setWorkspaceSettingsStatus: options.setWorkspaceSettingsStatus,
      setWorkspaceSettingsError: options.setWorkspaceSettingsError,
      onSaveWorkspaceSettings: options.onSaveWorkspaceSettings,
      onAssignMemberRole: options.onAssignMemberRole,
      onUnassignMemberRole: options.onUnassignMemberRole,
    },
    roleManagement: {
      hasActiveWorkspace: options.hasActiveWorkspace(),
      canManageWorkspaceRoles: options.canManageWorkspaceRoles(),
      canManageMemberRoles: options.canManageMemberRoles(),
      roles: options.roles(),
      isLoadingRoles: options.isLoadingRoles(),
      isMutatingRoles: options.isMutatingRoles(),
      roleManagementStatus: options.roleManagementStatus(),
      roleManagementError: options.roleManagementError(),
      targetUserIdInput: options.targetUserIdInput(),
      setTargetUserIdInput: options.setTargetUserIdInput,
      onRefreshRoles: options.onRefreshRoles,
      onCreateRole: options.onCreateRole,
      onUpdateRole: options.onUpdateRole,
      onDeleteRole: options.onDeleteRole,
      onReorderRoles: options.onReorderRoles,
      onAssignRole: options.onAssignRole,
      onUnassignRole: options.onUnassignRole,
      onOpenModerationPanel: options.onOpenModerationPanel,
    },
    utility: {
      echoInput: options.echoInput(),
      healthStatus: options.healthStatus(),
      diagError: options.diagError(),
      diagnosticsEventCounts: options.diagnosticsEventCounts(),
      showDiagnosticsCounters: options.showDiagnosticsCounters,
      isCheckingHealth: options.isCheckingHealth(),
      isEchoing: options.isEchoing(),
      setEchoInput: options.setEchoInput,
      onRunHealthCheck: options.onRunHealthCheck,
      onRunEcho: options.onRunEcho,
    },
  };
}
