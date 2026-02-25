import type {
  PermissionName,
  WorkspaceRoleId,
} from "../../../domain/chat";
import type {
  RoleManagementPanelBuilderOptions,
} from "../adapters/panel-host-props";

export interface RoleManagementPanelPropsOptions {
  hasActiveWorkspace: boolean;
  canManageWorkspaceRoles: boolean;
  canManageMemberRoles: boolean;
  roles: RoleManagementPanelBuilderOptions["roles"];
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
  }) => Promise<void> | void;
  onUpdateRole: (
    roleId: WorkspaceRoleId,
    input: {
      name?: string;
      permissions?: PermissionName[];
    },
  ) => Promise<void> | void;
  onDeleteRole: (roleId: WorkspaceRoleId) => Promise<void> | void;
  onReorderRoles: (roleIds: WorkspaceRoleId[]) => Promise<void> | void;
  onAssignRole: (targetUserIdInput: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignRole: (targetUserIdInput: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUpdateDefaultJoinRole?: (roleId: WorkspaceRoleId | null) => Promise<void> | void;
  onOpenModerationPanel: () => void;
}

export function createRoleManagementPanelProps(
  options: RoleManagementPanelPropsOptions,
): RoleManagementPanelBuilderOptions {
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
    setTargetUserIdInput: options.setTargetUserIdInput,
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
