import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import {
  guildVisibilityFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { createWorkspaceSettingsPanelProps } from "../src/features/app-shell/runtime/workspace-settings-panel-props";

describe("app shell workspace settings panel props", () => {
  it("maps workspace settings state and save action", () => {
    const onSaveWorkspaceSettings = vi.fn();
    const onAssignMemberRole = vi.fn();
    const onUnassignMemberRole = vi.fn();
    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      canManageMemberRoles: true,
      workspaceSettingsSection: "profile",
      workspaceName: "Filament",
      workspaceVisibility: guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: "ready",
      workspaceSettingsError: "",
      memberRoleStatus: "",
      memberRoleError: "",
      isMutatingMemberRoles: false,
      viewAsRoleSimulatorEnabled: false,
      viewAsRoleSimulatorRole: "member",
      members: [{ userId: "01ARZ3NDEKTSV4RRFFQ69G5FAX", label: "owner", roleIds: [] }],
      roles: [{
        roleId,
        name: workspaceRoleNameFromInput("Moderator"),
        position: 2,
        isSystem: false,
        permissions: [permissionFromInput("manage_member_roles")],
      }],
      assignableRoleIds: [roleId],
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: () => guildVisibilityFromInput("private"),
      setWorkspaceSettingsStatus: () => "",
      setWorkspaceSettingsError: () => "",
      setViewAsRoleSimulatorEnabled: () => undefined,
      setViewAsRoleSimulatorRole: () => undefined,
      onSaveWorkspaceSettings,
      onAssignMemberRole,
      onUnassignMemberRole,
    });

    expect(panelProps.hasActiveWorkspace).toBe(true);
    expect(panelProps.canManageWorkspaceSettings).toBe(true);
    expect(panelProps.canManageMemberRoles).toBe(true);
    expect(panelProps.workspaceSettingsSection).toBe("profile");
    expect(panelProps.viewAsRoleSimulatorEnabled).toBe(false);
    expect(panelProps.viewAsRoleSimulatorRole).toBe("member");
    expect(panelProps.workspaceName).toBe("Filament");
    expect(panelProps.workspaceVisibility).toBe("private");
    expect(panelProps.members).toHaveLength(1);

    panelProps.onSaveWorkspaceSettings();
    expect(onSaveWorkspaceSettings).toHaveBeenCalledTimes(1);
    void panelProps.onAssignMemberRole("01ARZ3NDEKTSV4RRFFQ69G5FAX", roleId);
    expect(onAssignMemberRole).toHaveBeenCalledOnce();
    void panelProps.onUnassignMemberRole("01ARZ3NDEKTSV4RRFFQ69G5FAX", roleId);
    expect(onUnassignMemberRole).toHaveBeenCalledOnce();
  });

  it("clears workspace settings status and error when name changes", () => {
    const [workspaceName, setWorkspaceName] = createSignal("Initial");
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("saved");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("error");

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      canManageMemberRoles: true,
      workspaceSettingsSection: "profile",
      workspaceName: workspaceName(),
      workspaceVisibility: guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: workspaceSettingsStatus(),
      workspaceSettingsError: workspaceSettingsError(),
      memberRoleStatus: "",
      memberRoleError: "",
      isMutatingMemberRoles: false,
      viewAsRoleSimulatorEnabled: false,
      viewAsRoleSimulatorRole: "member",
      members: [],
      roles: [],
      assignableRoleIds: [],
      setWorkspaceSettingsName: setWorkspaceName,
      setWorkspaceSettingsVisibility: () => guildVisibilityFromInput("private"),
      setWorkspaceSettingsStatus,
      setWorkspaceSettingsError,
      setViewAsRoleSimulatorEnabled: () => undefined,
      setViewAsRoleSimulatorRole: () => undefined,
      onSaveWorkspaceSettings: () => undefined,
      onAssignMemberRole: () => undefined,
      onUnassignMemberRole: () => undefined,
    });

    panelProps.setWorkspaceSettingsName("Updated");

    expect(workspaceName()).toBe("Updated");
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("");
  });

  it("clears workspace settings status and error when visibility changes", () => {
    const [workspaceVisibility, setWorkspaceVisibility] = createSignal(
      guildVisibilityFromInput("private"),
    );
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("saved");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("error");

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      canManageMemberRoles: true,
      workspaceSettingsSection: "profile",
      workspaceName: "Filament",
      workspaceVisibility: workspaceVisibility(),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: workspaceSettingsStatus(),
      workspaceSettingsError: workspaceSettingsError(),
      memberRoleStatus: "",
      memberRoleError: "",
      isMutatingMemberRoles: false,
      viewAsRoleSimulatorEnabled: false,
      viewAsRoleSimulatorRole: "member",
      members: [],
      roles: [],
      assignableRoleIds: [],
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: setWorkspaceVisibility,
      setWorkspaceSettingsStatus,
      setWorkspaceSettingsError,
      setViewAsRoleSimulatorEnabled: () => undefined,
      setViewAsRoleSimulatorRole: () => undefined,
      onSaveWorkspaceSettings: () => undefined,
      onAssignMemberRole: () => undefined,
      onUnassignMemberRole: () => undefined,
    });

    panelProps.setWorkspaceSettingsVisibility(guildVisibilityFromInput("public"));

    expect(workspaceVisibility()).toBe("public");
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("");
  });

  it("maps role simulator controls through workspace settings props", () => {
    const setViewAsRoleSimulatorEnabled = vi.fn();
    const setViewAsRoleSimulatorRole = vi.fn();

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      canManageMemberRoles: true,
      workspaceSettingsSection: "simulator",
      workspaceName: "Filament",
      workspaceVisibility: guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: "",
      workspaceSettingsError: "",
      memberRoleStatus: "",
      memberRoleError: "",
      isMutatingMemberRoles: false,
      viewAsRoleSimulatorEnabled: true,
      viewAsRoleSimulatorRole: "moderator",
      members: [],
      roles: [],
      assignableRoleIds: [],
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: () => undefined,
      setWorkspaceSettingsStatus: () => undefined,
      setWorkspaceSettingsError: () => undefined,
      setViewAsRoleSimulatorEnabled,
      setViewAsRoleSimulatorRole,
      onSaveWorkspaceSettings: () => undefined,
      onAssignMemberRole: () => undefined,
      onUnassignMemberRole: () => undefined,
    });

    panelProps.onViewAsRoleSimulatorToggle(false);
    panelProps.onViewAsRoleSimulatorRoleChange("owner");
    expect(setViewAsRoleSimulatorEnabled).toHaveBeenCalledWith(false);
    expect(setViewAsRoleSimulatorRole).toHaveBeenCalledWith("owner");
  });
});
