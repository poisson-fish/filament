import { describe, expect, it, vi } from "vitest";
import {
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { createRoleManagementPanelProps } from "../src/features/app-shell/runtime/role-management-panel-props";

describe("app shell role management panel props", () => {
  it("maps role management values and handlers", async () => {
    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
    const onRefreshRoles = vi.fn();
    const onCreateRole = vi.fn();
    const onUpdateRole = vi.fn();
    const onDeleteRole = vi.fn();
    const onReorderRoles = vi.fn();
    const onAssignRole = vi.fn();
    const onUnassignRole = vi.fn();

    const panelProps = createRoleManagementPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceRoles: true,
      canManageMemberRoles: true,
      roles: [
        {
          roleId,
          name: workspaceRoleNameFromInput("Moderator"),
          position: 10,
          isSystem: false,
          permissions: [permissionFromInput("manage_workspace_roles")],
        },
      ],
      isLoadingRoles: false,
      isMutatingRoles: false,
      roleManagementStatus: "ready",
      roleManagementError: "",
      targetUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
      setTargetUserIdInput: () => undefined,
      onRefreshRoles,
      onCreateRole,
      onUpdateRole,
      onDeleteRole,
      onReorderRoles,
      onAssignRole,
      onUnassignRole,
      onOpenModerationPanel: () => undefined,
    });

    expect(panelProps.hasActiveWorkspace).toBe(true);
    expect(panelProps.canManageWorkspaceRoles).toBe(true);
    expect(panelProps.canManageMemberRoles).toBe(true);
    expect(panelProps.roles).toHaveLength(1);

    await panelProps.onRefreshRoles();
    expect(onRefreshRoles).toHaveBeenCalledTimes(1);

    await panelProps.onCreateRole({ name: "Ops", permissions: ["create_message"] });
    expect(onCreateRole).toHaveBeenCalledWith({
      name: "Ops",
      permissions: ["create_message"],
    });

    await panelProps.onUpdateRole(roleId, { permissions: ["delete_message"] });
    expect(onUpdateRole).toHaveBeenCalledWith(roleId, {
      permissions: ["delete_message"],
    });

    await panelProps.onDeleteRole(roleId);
    expect(onDeleteRole).toHaveBeenCalledWith(roleId);

    await panelProps.onReorderRoles([roleId]);
    expect(onReorderRoles).toHaveBeenCalledWith([roleId]);

    await panelProps.onAssignRole("01ARZ3NDEKTSV4RRFFQ69G5FAZ", roleId);
    expect(onAssignRole).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAZ", roleId);

    await panelProps.onUnassignRole("01ARZ3NDEKTSV4RRFFQ69G5FAZ", roleId);
    expect(onUnassignRole).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAZ", roleId);
  });

  it("invokes moderation panel opener", () => {
    const onOpenModerationPanel = vi.fn();

    const panelProps = createRoleManagementPanelProps({
      hasActiveWorkspace: false,
      canManageWorkspaceRoles: false,
      canManageMemberRoles: false,
      roles: [],
      isLoadingRoles: false,
      isMutatingRoles: false,
      roleManagementStatus: "",
      roleManagementError: "",
      targetUserIdInput: "",
      setTargetUserIdInput: () => undefined,
      onRefreshRoles: () => undefined,
      onCreateRole: () => undefined,
      onUpdateRole: () => undefined,
      onDeleteRole: () => undefined,
      onReorderRoles: () => undefined,
      onAssignRole: () => undefined,
      onUnassignRole: () => undefined,
      onOpenModerationPanel,
    });

    panelProps.onOpenModerationPanel();
    expect(onOpenModerationPanel).toHaveBeenCalledTimes(1);
  });
});
