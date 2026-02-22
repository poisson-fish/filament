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
    workspaceName: "Filament",
    workspaceVisibility: "private",
    isSavingWorkspaceSettings: false,
    workspaceSettingsStatus: "",
    workspaceSettingsError: "",
    memberRoleStatus: "",
    memberRoleError: "",
    isMutatingMemberRoles: false,
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
    onWorkspaceNameInput: () => undefined,
    onWorkspaceVisibilityChange: () => undefined,
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
    expect(screen.getByLabelText("Workspace members search")).toHaveClass("border-line-soft");
    expect(screen.getByText("saved")).toHaveClass("text-ok");
    expect(screen.getByText("conflict")).toHaveClass("text-danger");

    expect(document.querySelector(".group-label")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".status")).toBeNull();
  });

  it("wires input and submit callbacks", async () => {
    const onWorkspaceNameInput = vi.fn();
    const onWorkspaceVisibilityChange = vi.fn();
    const onSaveWorkspaceSettings = vi.fn();

    render(() => (
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          onWorkspaceNameInput,
          onWorkspaceVisibilityChange,
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
});
