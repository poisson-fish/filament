import type { GuildVisibility, WorkspaceRoleId } from "../../../domain/chat";
import type { WorkspaceSettingsPanelBuilderOptions } from "../adapters/panel-host-props";

export interface WorkspaceSettingsPanelPropsOptions {
  hasActiveWorkspace: boolean;
  canManageWorkspaceSettings: boolean;
  canManageMemberRoles: boolean;
  workspaceName: string;
  workspaceVisibility: GuildVisibility;
  isSavingWorkspaceSettings: boolean;
  workspaceSettingsStatus: string;
  workspaceSettingsError: string;
  memberRoleStatus: string;
  memberRoleError: string;
  isMutatingMemberRoles: boolean;
  members: WorkspaceSettingsPanelBuilderOptions["members"];
  roles: WorkspaceSettingsPanelBuilderOptions["roles"];
  assignableRoleIds: WorkspaceSettingsPanelBuilderOptions["assignableRoleIds"];
  setWorkspaceSettingsName: (value: string) => void;
  setWorkspaceSettingsVisibility: (value: GuildVisibility) => void;
  setWorkspaceSettingsStatus: (value: string) => void;
  setWorkspaceSettingsError: (value: string) => void;
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
    workspaceName: options.workspaceName,
    workspaceVisibility: options.workspaceVisibility,
    isSavingWorkspaceSettings: options.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceSettingsError,
    memberRoleStatus: options.memberRoleStatus,
    memberRoleError: options.memberRoleError,
    isMutatingMemberRoles: options.isMutatingMemberRoles,
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
    onSaveWorkspaceSettings: options.onSaveWorkspaceSettings,
    onAssignMemberRole: options.onAssignMemberRole,
    onUnassignMemberRole: options.onUnassignMemberRole,
  };
}
