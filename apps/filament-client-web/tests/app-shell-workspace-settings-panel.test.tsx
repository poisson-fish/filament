import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  WorkspaceSettingsPanel,
  type WorkspaceSettingsPanelProps,
} from "../src/features/app-shell/components/panels/WorkspaceSettingsPanel";
import {
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";

function workspaceSettingsPanelPropsFixture(
  overrides: Partial<WorkspaceSettingsPanelProps> = {},
): WorkspaceSettingsPanelProps {
  return {
    hasActiveWorkspace: true,
    canManageWorkspaceSettings: true,
    canManageMemberRoles: true,
    activeSectionId: "profile",
    workspaceName: "Filament",
    workspaceVisibility: "private",
    isSavingWorkspaceSettings: false,
    workspaceSettingsStatus: "",
    workspaceSettingsError: "",
    memberRoleStatus: "",
    memberRoleError: "",
    isMutatingMemberRoles: false, isLoadingMembers: false, memberListError: "",
    viewAsRoleSimulatorEnabled: false,
    viewAsRoleSimulatorRole: "member",
    members: [
      {
        userId: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
        label: "owner",
        roleIds: [workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX")],
      },
    ],
    roles: [
      {
        roleId: workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX"),
        name: workspaceRoleNameFromInput("Moderator"),
        position: 40,
        isSystem: false,
        permissions: [permissionFromInput("manage_member_roles")],
      },
    ],
    assignableRoleIds: [workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX")],
    roleManagementPanelProps: {
      hasActiveWorkspace: true,
      canManageWorkspaceRoles: true,
      canManageMemberRoles: true,
      roles: [
        {
          roleId: workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX"),
          name: workspaceRoleNameFromInput("Moderator"),
          position: 40,
          isSystem: false,
          permissions: [permissionFromInput("manage_member_roles")],
        },
      ],
      isLoadingRoles: false,
      isMutatingRoles: false,
      roleManagementStatus: "",
      roleManagementError: "",
      targetUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
      onTargetUserIdInput: () => undefined,
      onRefreshRoles: () => undefined,
      onCreateRole: () => undefined,
      onUpdateRole: () => undefined,
      onDeleteRole: () => undefined,
      onReorderRoles: () => undefined,
      onAssignRole: () => undefined,
      onUnassignRole: () => undefined,
      onOpenModerationPanel: () => undefined,
    },
    onWorkspaceNameInput: () => undefined,
    onWorkspaceVisibilityChange: () => undefined,
    onViewAsRoleSimulatorToggle: () => undefined,
    onViewAsRoleSimulatorRoleChange: () => undefined,
    onSaveWorkspaceSettings: () => undefined,
    onAssignMemberRole: () => undefined,
    onUnassignMemberRole: () => undefined,
    ...overrides,
  };
}

describe("app shell workspace settings panel", () => {
  it("renders with utility classes and no legacy helper hooks", () => {
    render(() =>
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          workspaceSettingsStatus: "saved",
          workspaceSettingsError: "conflict",
        })}
      />
    );

    expect(screen.getByText("WORKSPACE")).toHaveClass("m-0");
    expect(screen.getByText("WORKSPACE")).toHaveClass("uppercase");
    expect(screen.getByLabelText("Workspace settings name")).toHaveClass("border-line-soft");
    expect(screen.getByLabelText("Workspace settings visibility")).toHaveClass("border-line-soft");
    expect(screen.getByRole("button", { name: "Save workspace" })).toHaveClass("border-line-soft");
    expect(screen.getByText("saved")).toHaveClass("text-ok");
    expect(screen.getByText("conflict")).toHaveClass("text-danger");
    fireEvent.click(screen.getByRole("button", { name: "Open Members workspace section" }));
    expect(screen.getByLabelText("Workspace members search")).toHaveClass("border-line-soft");

    expect(document.querySelector(".group-label")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".status")).toBeNull();
  });

  it("wires input and submit callbacks", async () => {
    const onWorkspaceNameInput = vi.fn();
    const onWorkspaceVisibilityChange = vi.fn();
    const onViewAsRoleSimulatorToggle = vi.fn();
    const onViewAsRoleSimulatorRoleChange = vi.fn();
    const onSaveWorkspaceSettings = vi.fn();

    render(() => (
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          onWorkspaceNameInput,
          onWorkspaceVisibilityChange,
          onViewAsRoleSimulatorToggle,
          onViewAsRoleSimulatorRoleChange,
          onSaveWorkspaceSettings,
        })}
      />
    ));

    await fireEvent.input(screen.getByLabelText("Workspace settings name"), {
      target: { value: "Filament Updated" },
    });
    expect(onWorkspaceNameInput).toHaveBeenCalledWith("Filament Updated");

    await fireEvent.change(screen.getByLabelText("Workspace settings visibility"), {
      target: { value: "public" },
    });
    expect(onWorkspaceVisibilityChange).toHaveBeenCalledWith("public");

    await fireEvent.click(
      screen.getByRole("button", { name: "Open Permission Simulator workspace section" }),
    );
    await fireEvent.click(screen.getByLabelText("Enable view server as role simulator"));
    expect(onViewAsRoleSimulatorToggle).toHaveBeenCalledWith(true);

    await fireEvent.change(screen.getByLabelText("Workspace role simulator selection"), {
      target: { value: "moderator" },
    });
    expect(onViewAsRoleSimulatorRoleChange).toHaveBeenCalledWith("moderator");

    await fireEvent.click(screen.getByRole("button", { name: "Open Server Profile workspace section" }));
    const form = screen.getByRole("button", { name: "Save workspace" }).closest("form");
    expect(form).not.toBeNull();
    await fireEvent.submit(form!);
    expect(onSaveWorkspaceSettings).toHaveBeenCalledTimes(1);
  });

  it("supports inline member role assignment and unassignment actions", async () => {
    const onAssignMemberRole = vi.fn();
    const onUnassignMemberRole = vi.fn();

    render(() => (
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          onAssignMemberRole,
          onUnassignMemberRole,
        })}
      />
    ));

    await fireEvent.click(screen.getByRole("button", { name: "Open Members workspace section" }));
    await fireEvent.click(screen.getByRole("button", { name: "Assign role" }));
    expect(onAssignMemberRole).toHaveBeenCalledWith(
      "01ARZ3NDEKTSV4RRFFQ69G5FAW",
      "01ARZ3NDEKTSV4RRFFQ69G5FAX",
    );

    await fireEvent.click(
      screen.getByRole("button", {
        name: "Unassign Moderator from owner",
      }),
    );
    expect(onUnassignMemberRole).toHaveBeenCalledWith(
      "01ARZ3NDEKTSV4RRFFQ69G5FAW",
      "01ARZ3NDEKTSV4RRFFQ69G5FAX",
    );
  });

  it("renders embedded role management controls under the roles section", async () => {
    render(() => <WorkspaceSettingsPanel {...workspaceSettingsPanelPropsFixture()} />);

    await fireEvent.click(
      screen.getByRole("button", { name: "Open Roles & Hierarchy workspace section" }),
    );

    expect(screen.getByRole("button", { name: "Create role" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh roles" })).toBeInTheDocument();
  });

  it("syncs section changes to host state for rerender stability", async () => {
    const setWorkspaceSettingsSection = vi.fn();
    render(() => (
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          setWorkspaceSettingsSection,
        })}
      />
    ));

    await fireEvent.click(
      screen.getByRole("button", { name: "Open Roles & Hierarchy workspace section" }),
    );
    expect(setWorkspaceSettingsSection).toHaveBeenCalledWith("roles");
  });
});
