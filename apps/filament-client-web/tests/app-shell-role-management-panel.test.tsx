import { fireEvent, render, screen, within } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { RoleManagementPanel } from "../src/features/app-shell/components/panels/RoleManagementPanel";

const ROLE_ID = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

function panelProps(overrides: Partial<Parameters<typeof RoleManagementPanel>[0]> = {}) {
  return {
    hasActiveWorkspace: true,
    canManageWorkspaceRoles: true,
    canManageMemberRoles: true,
    roles: [
      {
        roleId: ROLE_ID,
        name: workspaceRoleNameFromInput("Responder"),
        position: 3,
        isSystem: false,
        permissions: [permissionFromInput("create_message")],
      },
    ],
    isLoadingRoles: false,
    isMutatingRoles: false,
    roleManagementStatus: "",
    roleManagementError: "",
    targetUserIdInput: "",
    onTargetUserIdInput: vi.fn(),
    onRefreshRoles: vi.fn(),
    onCreateRole: vi.fn(),
    onUpdateRole: vi.fn(),
    onDeleteRole: vi.fn(),
    onReorderRoles: vi.fn(),
    onAssignRole: vi.fn(),
    onUnassignRole: vi.fn(),
    onOpenModerationPanel: vi.fn(),
    ...overrides,
  };
}

describe("role management panel", () => {
  it("submits permission matrix create and assignment actions", async () => {
    const onCreateRole = vi.fn(async () => undefined);
    const onAssignRole = vi.fn(async () => undefined);
    const onTargetUserIdInput = vi.fn();

    render(() =>
      RoleManagementPanel(
        panelProps({
          onCreateRole,
          onAssignRole,
          onTargetUserIdInput,
          targetUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
        }),
      ),
    );

    fireEvent.input(screen.getAllByLabelText("Role name")[0]!, {
      target: { value: "Incident Lead" },
    });

    const createMatrix = screen.getByLabelText("create role permission matrix");
    const manageWorkspaceRolesToggle = within(createMatrix).getByLabelText(
      /Manage Workspace Roles/i,
    );
    fireEvent.click(manageWorkspaceRolesToggle);

    fireEvent.click(screen.getByRole("button", { name: "Create role" }));

    expect(onCreateRole).toHaveBeenCalledWith({
      name: "Incident Lead",
      permissions: [
        "create_message",
        "manage_workspace_roles",
        "subscribe_streams",
      ],
    });

    fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: "01ARZ3NDEKTSV4RRFFQ69G5FAW" },
    });
    expect(onTargetUserIdInput).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAW");

    fireEvent.click(screen.getByRole("button", { name: "Assign role" }));
    expect(onAssignRole).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAW", ROLE_ID);
  });

  it("applies role templates to the create draft and submits template permissions", async () => {
    const onCreateRole = vi.fn(async () => undefined);

    render(() =>
      RoleManagementPanel(
        panelProps({
          onCreateRole,
        }),
      ),
    );

    const createMatrix = screen.getByLabelText("create role permission matrix");
    const roleTemplateSelect = screen.getByLabelText("Role template");

    fireEvent.change(roleTemplateSelect, {
      target: { value: "moderator" },
    });

    expect(screen.getAllByLabelText("Role name")[0]).toHaveValue("Moderator");
    expect(within(createMatrix).getByLabelText(/Delete Messages/i)).toBeChecked();
    expect(within(createMatrix).getByLabelText(/Ban Members/i)).toBeChecked();
    expect(within(createMatrix).getByLabelText(/Publish Camera/i)).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Create role" }));

    expect(onCreateRole).toHaveBeenCalledWith({
      name: "Moderator",
      permissions: [
        "create_message",
        "delete_message",
        "ban_member",
        "view_audit_log",
        "manage_ip_bans",
        "subscribe_streams",
      ],
    });
  });

  it("hides management controls when role permissions are missing", () => {
    render(() =>
      RoleManagementPanel(
        panelProps({
          canManageWorkspaceRoles: false,
          canManageMemberRoles: false,
        }),
      ),
    );

    expect(screen.queryByRole("button", { name: "Create role" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Assign role" })).not.toBeInTheDocument();
  });

  it("renders hierarchy and permission matrix with utility classes and no legacy hooks", () => {
    render(() => RoleManagementPanel(panelProps()));

    const hierarchy = screen.getByLabelText("role hierarchy");
    expect(hierarchy).toHaveClass("grid");

    const refreshButton = screen.getByRole("button", { name: "Refresh roles" });
    expect(refreshButton.className).toContain("flex-1");

    const selectedRoleButton = screen.getByRole("button", { name: /Responder/ });
    expect(selectedRoleButton.className).toContain("border-brand");

    const createMatrix = screen.getByLabelText("create role permission matrix");
    expect(createMatrix).toHaveClass("grid");
    const firstToggle = within(createMatrix).getByLabelText(/Create Messages/i).closest("label");
    expect(firstToggle?.className).toContain("grid-cols-[auto_1fr]");

    expect(document.querySelector(".role-hierarchy-grid")).toBeNull();
    expect(document.querySelector(".role-hierarchy-item")).toBeNull();
    expect(document.querySelector(".permission-grid")).toBeNull();
    expect(document.querySelector(".permission-toggle")).toBeNull();
    expect(document.querySelector(".member-group")).toBeNull();
    expect(document.querySelector(".button-row")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
  });

  it("renders system role badge without the legacy status-chip hook", () => {
    render(() =>
      RoleManagementPanel(
        panelProps({
          roles: [
            {
              roleId: ROLE_ID,
              name: workspaceRoleNameFromInput("Owner"),
              position: 10,
              isSystem: true,
              permissions: [permissionFromInput("manage_workspace_roles")],
            },
          ],
        }),
      ),
    );

    const systemBadge = screen.getByText("system");
    expect(systemBadge.className).toContain("uppercase");
    expect(systemBadge).not.toHaveClass("status-chip");
    expect(document.querySelector(".status-chip")).toBeNull();
  });

  it("requires explicit confirmation before delete action", async () => {
    const onDeleteRole = vi.fn(async () => undefined);
    const confirmMock = vi.spyOn(window, "confirm").mockReturnValue(false);

    render(() => RoleManagementPanel(panelProps({ onDeleteRole })));

    fireEvent.click(screen.getByRole("button", { name: "Delete role" }));

    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(onDeleteRole).not.toHaveBeenCalled();
    confirmMock.mockRestore();
  });

  it("saves only changed role fields and trims role names", async () => {
    const onUpdateRole = vi.fn(async () => undefined);

    render(() =>
      RoleManagementPanel(
        panelProps({
          onUpdateRole,
          roles: [
            {
              roleId: ROLE_ID,
              name: workspaceRoleNameFromInput("Responder"),
              position: 3,
              isSystem: false,
              permissions: [
                permissionFromInput("create_message"),
                permissionFromInput("subscribe_streams"),
              ],
            },
          ],
        }),
      ),
    );

    fireEvent.input(screen.getAllByLabelText("Role name")[1]!, {
      target: { value: " Incident Lead " },
    });
    const editMatrix = screen.getByLabelText("edit role permission matrix");
    fireEvent.click(within(editMatrix).getByLabelText(/Delete Messages/i));

    fireEvent.click(screen.getByRole("button", { name: "Save role" }));

    expect(onUpdateRole).toHaveBeenCalledWith(ROLE_ID, {
      name: "Incident Lead",
      permissions: ["create_message", "delete_message", "subscribe_streams"],
    });
  });

  it("disables saving when no edits exist and supports draft reset", async () => {
    const onUpdateRole = vi.fn(async () => undefined);

    render(() =>
      RoleManagementPanel(
        panelProps({
          onUpdateRole,
        }),
      ),
    );

    const saveButton = screen.getByRole("button", { name: "Save role" });
    expect(saveButton).toBeDisabled();

    fireEvent.input(screen.getAllByLabelText("Role name")[1]!, {
      target: { value: "Responder Prime" },
    });

    expect(screen.getByText("unsaved changes")).toBeInTheDocument();
    expect(saveButton).not.toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Reset draft" }));

    expect(screen.getAllByLabelText("Role name")[1]).toHaveValue("Responder");
    expect(saveButton).toBeDisabled();
    expect(onUpdateRole).not.toHaveBeenCalled();
  });

  it("reorders custom roles via drag-and-drop and submits the updated hierarchy", async () => {
    const onReorderRoles = vi.fn(async () => undefined);
    const confirmMock = vi.spyOn(window, "confirm").mockReturnValue(true);
    const incidentRoleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
    const observerRoleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

    render(() =>
      RoleManagementPanel(
        panelProps({
          onReorderRoles,
          roles: [
            {
              roleId: ROLE_ID,
              name: workspaceRoleNameFromInput("Responder"),
              position: 8,
              isSystem: false,
              permissions: [permissionFromInput("create_message")],
            },
            {
              roleId: incidentRoleId,
              name: workspaceRoleNameFromInput("Incident Lead"),
              position: 6,
              isSystem: false,
              permissions: [permissionFromInput("manage_member_roles")],
            },
            {
              roleId: observerRoleId,
              name: workspaceRoleNameFromInput("Observer"),
              position: 4,
              isSystem: false,
              permissions: [permissionFromInput("subscribe_streams")],
            },
          ],
        }),
      ),
    );

    const incidentReorderRow = screen.getByLabelText("Reorder role Incident Lead");
    const responderReorderRow = screen.getByLabelText("Reorder role Responder");

    fireEvent.dragStart(incidentReorderRow);
    fireEvent.dragOver(responderReorderRow);
    fireEvent.drop(responderReorderRow);
    fireEvent.dragEnd(incidentReorderRow);

    fireEvent.click(screen.getByRole("button", { name: "Save hierarchy order" }));

    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(onReorderRoles).toHaveBeenCalledWith([incidentRoleId, ROLE_ID, observerRoleId]);
    confirmMock.mockRestore();
  });
});
