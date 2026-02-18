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
        "subscribe_streams",
        "manage_workspace_roles",
      ],
    });

    fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: "01ARZ3NDEKTSV4RRFFQ69G5FAW" },
    });
    expect(onTargetUserIdInput).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAW");

    fireEvent.click(screen.getByRole("button", { name: "Assign role" }));
    expect(onAssignRole).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAW", ROLE_ID);
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
});
