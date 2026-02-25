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

    fireEvent.click(screen.getByRole("button", { name: "Create role" }));

    fireEvent.input(screen.getByLabelText("Role name"), {
      target: { value: "Incident Lead" },
    });

    const createMatrix = screen.getByLabelText("create role permission matrix");
    const manageWorkspaceRolesToggle = within(createMatrix).getByLabelText(
      /Manage Workspace Roles/i,
    );
    fireEvent.click(manageWorkspaceRolesToggle);

    fireEvent.click(screen.getByRole("button", { name: "Create Role" }));

    expect(onCreateRole).toHaveBeenCalledWith({
      name: "Incident Lead",
      permissions: [
        "create_message",
        "manage_workspace_roles",
        "subscribe_streams",
      ],
    });

    fireEvent.click(await screen.findByRole("button", { name: "Manage Members" }));

    fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: "01ARZ3NDEKTSV4RRFFQ69G5FAW" },
    });
    expect(onTargetUserIdInput).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAW");

    fireEvent.click(screen.getByRole("button", { name: "Assign Role" }));
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

    fireEvent.click(screen.getByRole("button", { name: "Create role" }));

    const createMatrix = screen.getByLabelText("create role permission matrix");
    const roleTemplateSelect = screen.getByLabelText("Role template");

    fireEvent.change(roleTemplateSelect, {
      target: { value: "moderator" },
    });

    expect(screen.getByLabelText("Role name")).toHaveValue("Moderator");
    expect(within(createMatrix).getByLabelText(/Delete Messages/i)).toBeChecked();
    expect(within(createMatrix).getByLabelText(/Ban Members/i)).toBeChecked();
    expect(within(createMatrix).getByLabelText(/Publish Camera/i)).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Create Role" }));

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
    fireEvent.click(screen.getByRole("button", { name: "Manage Members" }));
    expect(screen.getByRole("button", { name: "Assign Role" })).toBeDisabled();
  });

  it("renders hierarchy and permission matrix with utility classes and no legacy hooks", () => {
    render(() => RoleManagementPanel(panelProps()));

    fireEvent.click(screen.getByRole("button", { name: "Create role" }));

    const refreshButton = screen.getByRole("button", { name: "Refresh roles" });
    expect(refreshButton).toHaveClass("border-line-soft");

    const createMatrix = screen.getByLabelText("create role permission matrix");
    expect(createMatrix).toHaveClass("grid");
    const firstToggle = within(createMatrix).getByLabelText(/Create Messages/i).closest("label");
    expect(firstToggle?.className).toContain("flex");
    expect(
      within(createMatrix).getByText(
        "Create, update, delete, and reorder workspace roles.",
      ),
    ).toBeInTheDocument();

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

    const systemBadges = screen.getAllByText("System");
    const systemBadge = systemBadges.find((node) =>
      node.className.includes("rounded-full"),
    ) as HTMLElement | undefined;
    expect(systemBadge).toBeTruthy();
    expect(systemBadge!).not.toHaveClass("status-chip");
    expect(document.querySelector(".status-chip")).toBeNull();
  });

  it("requires explicit confirmation before delete action", async () => {
    const onDeleteRole = vi.fn(async () => undefined);

    render(() => RoleManagementPanel(panelProps({ onDeleteRole })));

    fireEvent.click(screen.getByRole("button", { name: "Delete Role" }));

    const deleteDialog = screen.getByRole("dialog", {
      name: "Dangerous operation confirmation",
    });
    expect(within(deleteDialog).getByRole("heading", { name: "Delete role?" })).toBeInTheDocument();
    fireEvent.click(within(deleteDialog).getByRole("button", { name: "Cancel" }));
    expect(onDeleteRole).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Delete Role" }));
    const confirmedDeleteDialog = screen.getByRole("dialog", {
      name: "Dangerous operation confirmation",
    });
    fireEvent.click(within(confirmedDeleteDialog).getByRole("button", { name: /^Delete role$/ }));

    expect(onDeleteRole).toHaveBeenCalledWith(ROLE_ID);
  });

  it("opens delete confirmation from the role list delete control", async () => {
    const onDeleteRole = vi.fn(async () => undefined);

    render(() => RoleManagementPanel(panelProps({ onDeleteRole })));

    fireEvent.click(screen.getByRole("button", { name: "Delete role Responder" }));

    const deleteDialog = screen.getByRole("dialog", {
      name: "Dangerous operation confirmation",
    });
    expect(within(deleteDialog).getByRole("heading", { name: "Delete role?" })).toBeInTheDocument();
    fireEvent.click(within(deleteDialog).getByRole("button", { name: /^Delete role$/ }));

    expect(onDeleteRole).toHaveBeenCalledWith(ROLE_ID);
  });

  it("warns and requires modal confirmation for dangerous permission changes", async () => {
    const onUpdateRole = vi.fn(async () => undefined);

    render(() =>
      RoleManagementPanel(
        panelProps({
          onUpdateRole,
        }),
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Permissions" }));
    const editMatrix = screen.getByLabelText("edit role permission matrix");
    fireEvent.click(within(editMatrix).getByLabelText(/Manage Workspace Roles/i));

    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));

    expect(
      screen.getByRole("heading", {
        name: "Confirm dangerous permission change",
      }),
    ).toBeInTheDocument();
    expect(onUpdateRole).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Apply dangerous change" }));

    expect(onUpdateRole).toHaveBeenCalledWith(ROLE_ID, {
      permissions: ["create_message", "manage_workspace_roles"],
    });
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

    fireEvent.input(screen.getByLabelText("Role name"), {
      target: { value: " Incident Lead " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Permissions" }));
    const editMatrix = screen.getByLabelText("edit role permission matrix");
    fireEvent.click(within(editMatrix).getByLabelText(/Delete Messages/i));

    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));

    expect(onUpdateRole).toHaveBeenCalledWith(ROLE_ID, {
      name: "Incident Lead",
      permissions: ["create_message", "delete_message", "subscribe_streams"],
    });
  });

  it("saves only role name from display actions", async () => {
    const onUpdateRole = vi.fn(async () => undefined);

    render(() =>
      RoleManagementPanel(
        panelProps({
          onUpdateRole,
        }),
      ),
    );

    fireEvent.input(screen.getByLabelText("Role name"), {
      target: { value: "  Captain  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save name" }));

    expect(onUpdateRole).toHaveBeenCalledWith(ROLE_ID, {
      name: "Captain",
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

    expect(screen.queryByRole("button", { name: "Save Changes" })).not.toBeInTheDocument();

    fireEvent.input(screen.getByLabelText("Role name"), {
      target: { value: "Responder Prime" },
    });

    expect(screen.getByText(/^unsaved changes$/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save Changes" })).toBeInTheDocument();

    const unsavedBanner = screen
      .getByText(/you have unsaved changes to this role/i)
      .closest("div");
    fireEvent.click(within(unsavedBanner!).getByRole("button", { name: "Reset" }));

    expect(screen.getByLabelText("Role name")).toHaveValue("Responder");
    expect(screen.queryByRole("button", { name: "Save Changes" })).not.toBeInTheDocument();
    expect(onUpdateRole).not.toHaveBeenCalled();
  });

  it("reorders custom roles via drag-and-drop and submits the updated hierarchy", async () => {
    const onReorderRoles = vi.fn(async () => undefined);
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

    const incidentReorderRow = screen.getByRole("button", { name: /^incident lead$/i });
    const responderReorderRow = screen.getByRole("button", { name: /^responder$/i });

    fireEvent.dragStart(incidentReorderRow);
    fireEvent.dragOver(responderReorderRow);
    fireEvent.drop(responderReorderRow);
    fireEvent.dragEnd(incidentReorderRow);

    const reorderNotice = screen.getByText(/Careful - you have unsaved changes!/i).closest("div");
    fireEvent.click(within(reorderNotice!).getByRole("button", { name: "Save" }));

    const reorderDialog = screen.getByRole("dialog", {
      name: "Dangerous operation confirmation",
    });
    expect(
      within(reorderDialog).getByRole("heading", { name: "Apply hierarchy reorder?" }),
    ).toBeInTheDocument();
    fireEvent.click(within(reorderDialog).getByRole("button", { name: /^Save hierarchy order$/ }));

    expect(onReorderRoles).toHaveBeenCalledWith([incidentRoleId, ROLE_ID, observerRoleId]);
  });
});
