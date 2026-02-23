import type { GuildVisibility, RoleName, WorkspaceRoleId } from "../../../domain/chat";
import type { WorkspaceSettingsSection } from "../types";
import type { WorkspaceSettingsPanelBuilderOptions } from "../adapters/panel-host-props";

export interface WorkspaceSettingsPanelPropsOptions {
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
  viewAsRoleSimulatorEnabled: boolean;
  viewAsRoleSimulatorRole: RoleName;
  members: WorkspaceSettingsPanelBuilderOptions["members"];
  roles: WorkspaceSettingsPanelBuilderOptions["roles"];
  assignableRoleIds: WorkspaceSettingsPanelBuilderOptions["assignableRoleIds"];
  setWorkspaceSettingsName: (value: string) => void;
  setWorkspaceSettingsVisibility: (value: GuildVisibility) => void;
  setWorkspaceSettingsStatus: (value: string) => void;
  setWorkspaceSettingsError: (value: string) => void;
  setViewAsRoleSimulatorEnabled: (value: boolean) => void;
  setViewAsRoleSimulatorRole: (value: RoleName) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
  onAssignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
}

export function createWorkspaceSettingsPanelProps(
  options: WorkspaceSettingsPanelPropsOptions,
): WorkspaceSettingsPanelBuilderOptions {
  return {
    hasActiveWorkspace: options.hasActiveWorkspace,
    canManageWorkspaceSettings: options.canManageWorkspaceSettings,
    canManageMemberRoles: options.canManageMemberRoles,
    workspaceSettingsSection: options.workspaceSettingsSection,
    workspaceName: options.workspaceName,
    workspaceVisibility: options.workspaceVisibility,
    isSavingWorkspaceSettings: options.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceSettingsError,
    memberRoleStatus: options.memberRoleStatus,
    memberRoleError: options.memberRoleError,
    isMutatingMemberRoles: options.isMutatingMemberRoles,
    viewAsRoleSimulatorEnabled: options.viewAsRoleSimulatorEnabled,
    viewAsRoleSimulatorRole: options.viewAsRoleSimulatorRole,
    members: options.members,
    roles: options.roles,
    assignableRoleIds: options.assignableRoleIds,
    setWorkspaceSettingsName: (value) => {
      options.setWorkspaceSettingsName(value);
      options.setWorkspaceSettingsStatus("");
      options.setWorkspaceSettingsError("");
    },
    setWorkspaceSettingsVisibility: (value) => {
      options.setWorkspaceSettingsVisibility(value);
      options.setWorkspaceSettingsStatus("");
      options.setWorkspaceSettingsError("");
    },
    onViewAsRoleSimulatorToggle: options.setViewAsRoleSimulatorEnabled,
    onViewAsRoleSimulatorRoleChange: options.setViewAsRoleSimulatorRole,
    onSaveWorkspaceSettings: options.onSaveWorkspaceSettings,
    onAssignMemberRole: options.onAssignMemberRole,
    onUnassignMemberRole: options.onUnassignMemberRole,
  };
}
