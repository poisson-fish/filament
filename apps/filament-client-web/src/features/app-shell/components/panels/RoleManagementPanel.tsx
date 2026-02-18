import { For, Show, createEffect, createMemo, createSignal } from "solid-js";
import {
  type PermissionName,
  type GuildRoleRecord,
  type WorkspaceRoleId,
} from "../../../../domain/chat";

interface PermissionMatrixEntry {
  permission: PermissionName;
  label: string;
  summary: string;
}

const PERMISSION_MATRIX: PermissionMatrixEntry[] = [
  {
    permission: "create_message",
    label: "Create Messages",
    summary: "Send messages and participate in channels.",
  },
  {
    permission: "delete_message",
    label: "Delete Messages",
    summary: "Delete or edit messages authored by other members.",
  },
  {
    permission: "manage_channel_overrides",
    label: "Manage Overrides",
    summary: "Edit channel role override rules.",
  },
  {
    permission: "ban_member",
    label: "Ban Members",
    summary: "Kick and ban users at workspace scope.",
  },
  {
    permission: "manage_member_roles",
    label: "Manage Member Roles",
    summary: "Assign and unassign workspace roles on members.",
  },
  {
    permission: "manage_workspace_roles",
    label: "Manage Workspace Roles",
    summary: "Create, update, delete, and reorder workspace roles.",
  },
  {
    permission: "view_audit_log",
    label: "View Audit Log",
    summary: "Read redacted workspace audit history.",
  },
  {
    permission: "manage_ip_bans",
    label: "Manage IP Bans",
    summary: "Apply and remove user-derived guild IP bans.",
  },
  {
    permission: "publish_video",
    label: "Publish Camera",
    summary: "Publish camera tracks in voice channels.",
  },
  {
    permission: "publish_screen_share",
    label: "Publish Screen",
    summary: "Publish screen-share tracks in voice channels.",
  },
  {
    permission: "subscribe_streams",
    label: "Subscribe Streams",
    summary: "Receive remote media streams in voice channels.",
  },
  {
    permission: "manage_roles",
    label: "Legacy Manage Roles",
    summary: "Compatibility grant for pre-phase-7 moderation paths.",
  },
];

export interface RoleManagementPanelProps {
  hasActiveWorkspace: boolean;
  canManageWorkspaceRoles: boolean;
  canManageMemberRoles: boolean;
  roles: GuildRoleRecord[];
  isLoadingRoles: boolean;
  isMutatingRoles: boolean;
  roleManagementStatus: string;
  roleManagementError: string;
  targetUserIdInput: string;
  onTargetUserIdInput: (value: string) => void;
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
  onOpenModerationPanel: () => void;
}

function togglePermission(
  list: PermissionName[],
  permission: PermissionName,
  checked: boolean,
): PermissionName[] {
  if (checked) {
    if (list.includes(permission)) {
      return list;
    }
    return [...list, permission];
  }
  return list.filter((entry) => entry !== permission);
}

function sortRolesByHierarchy(roles: GuildRoleRecord[]): GuildRoleRecord[] {
  return [...roles].sort((left, right) => right.position - left.position);
}

function hasManagementPermission(permissions: PermissionName[]): boolean {
  return (
    permissions.includes("manage_workspace_roles") ||
    permissions.includes("manage_member_roles") ||
    permissions.includes("manage_roles")
  );
}

export function RoleManagementPanel(props: RoleManagementPanelProps) {
  const [clientError, setClientError] = createSignal("");

  const [createName, setCreateName] = createSignal("Responder");
  const [createPermissions, setCreatePermissions] = createSignal<PermissionName[]>([
    "create_message",
    "subscribe_streams",
  ]);

  const [selectedRoleId, setSelectedRoleId] = createSignal<WorkspaceRoleId | null>(null);
  const [editName, setEditName] = createSignal("");
  const [editPermissions, setEditPermissions] = createSignal<PermissionName[]>([]);
  const [confirmRiskyRoleEdit, setConfirmRiskyRoleEdit] = createSignal(false);

  const [reorderDraftRoleIds, setReorderDraftRoleIds] = createSignal<WorkspaceRoleId[]>([]);

  const [assignmentRoleId, setAssignmentRoleId] = createSignal<WorkspaceRoleId | null>(null);
  const roleHierarchyClass = "grid gap-[0.5rem]";
  const roleHierarchyItemClass =
    "grid gap-[0.12rem] rounded-[0.62rem] border border-line-soft bg-bg-2 px-[0.58rem] py-[0.52rem] text-left text-ink-1";
  const roleHierarchyMetaClass = "m-0 text-[0.82rem] text-ink-2";
  const permissionGridClass = "grid gap-[0.5rem]";
  const permissionToggleClass =
    "grid grid-cols-[auto_1fr] items-start gap-x-[0.52rem] gap-y-[0.2rem] rounded-[0.62rem] border border-line-soft bg-bg-1 px-[0.6rem] py-[0.5rem]";
  const checkboxRowClass =
    "flex items-center gap-[0.5rem] [&>input]:mt-[0.14rem] [&>input]:h-[0.95rem] [&>input]:w-[0.95rem]";
  const statusChipClass =
    "inline-block text-[0.7rem] uppercase tracking-[0.06em] text-ink-2";
  const rolePreviewClass = "m-0 break-words text-ink-2";
  const statusBaseClass = "mt-[0.92rem] text-[0.91rem]";

  const hierarchyRoles = createMemo(() => sortRolesByHierarchy(props.roles));
  const assignableRoles = createMemo(() =>
    hierarchyRoles().filter((role) => !role.isSystem),
  );

  const selectedRole = createMemo<GuildRoleRecord | null>(() => {
    const roleId = selectedRoleId();
    if (!roleId) {
      return null;
    }
    return hierarchyRoles().find((entry) => entry.roleId === roleId) ?? null;
  });

  const editableRole = createMemo<GuildRoleRecord | null>(() => {
    const role = selectedRole();
    if (!role || role.isSystem) {
      return null;
    }
    return role;
  });

  const createPreview = createMemo(() =>
    PERMISSION_MATRIX.filter((entry) => createPermissions().includes(entry.permission)),
  );
  const editPreview = createMemo(() =>
    PERMISSION_MATRIX.filter((entry) => editPermissions().includes(entry.permission)),
  );

  const isRiskyPermissionDrop = createMemo(() => {
    const role = editableRole();
    if (!role) {
      return false;
    }
    return (
      hasManagementPermission(role.permissions) &&
      !hasManagementPermission(editPermissions())
    );
  });

  createEffect(() => {
    const roles = hierarchyRoles();
    if (roles.length === 0) {
      setSelectedRoleId(null);
      return;
    }
    const current = selectedRoleId();
    if (!current || !roles.some((entry) => entry.roleId === current)) {
      setSelectedRoleId(roles[0]!.roleId);
    }
  });

  createEffect(() => {
    const role = selectedRole();
    if (!role) {
      setEditName("");
      setEditPermissions([]);
      setConfirmRiskyRoleEdit(false);
      return;
    }
    setEditName(role.name);
    setEditPermissions(role.permissions);
    setConfirmRiskyRoleEdit(false);
  });

  createEffect(() => {
    const reorderable = hierarchyRoles()
      .filter((role) => !role.isSystem)
      .map((role) => role.roleId);
    setReorderDraftRoleIds(reorderable);
  });

  createEffect(() => {
    const roles = assignableRoles();
    const current = assignmentRoleId();
    if (roles.length === 0) {
      setAssignmentRoleId(null);
      return;
    }
    if (!current || !roles.some((entry) => entry.roleId === current)) {
      setAssignmentRoleId(roles[0]!.roleId);
    }
  });

  const invoke = async (operation: () => Promise<void> | void): Promise<void> => {
    setClientError("");
    try {
      await operation();
    } catch (error) {
      setClientError(error instanceof Error ? error.message : "Role operation failed.");
    }
  };

  const onCreateRole = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    if (!props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }

    await invoke(async () => {
      await props.onCreateRole({
        name: createName(),
        permissions: createPermissions(),
      });
      setCreateName("Responder");
      setCreatePermissions(["create_message", "subscribe_streams"]);
    });
  };

  const onSaveRole = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    const role = editableRole();
    if (!role || !props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    if (isRiskyPermissionDrop() && !confirmRiskyRoleEdit()) {
      setClientError(
        "Confirm risky permission reduction before applying this role change.",
      );
      return;
    }

    await invoke(async () => {
      await props.onUpdateRole(role.roleId, {
        name: editName(),
        permissions: editPermissions(),
      });
    });
  };

  const onDeleteRole = async (): Promise<void> => {
    const role = editableRole();
    if (!role || !props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    if (!window.confirm(`Delete role ${role.name}? This cannot be undone.`)) {
      return;
    }
    await invoke(async () => {
      await props.onDeleteRole(role.roleId);
    });
  };

  const moveRole = (roleId: WorkspaceRoleId, direction: "up" | "down"): void => {
    setReorderDraftRoleIds((current) => {
      const index = current.findIndex((value) => value === roleId);
      if (index < 0) {
        return current;
      }
      const target = direction === "up" ? index - 1 : index + 1;
      if (target < 0 || target >= current.length) {
        return current;
      }
      const next = [...current];
      const [moved] = next.splice(index, 1);
      next.splice(target, 0, moved!);
      return next;
    });
  };

  const onSaveReorder = async (): Promise<void> => {
    if (!props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    if (!window.confirm("Apply this role hierarchy reorder?")) {
      return;
    }
    await invoke(async () => {
      await props.onReorderRoles(reorderDraftRoleIds());
    });
  };

  const onAssignRole = async (): Promise<void> => {
    const roleId = assignmentRoleId();
    if (!roleId || !props.canManageMemberRoles || !props.hasActiveWorkspace) {
      return;
    }
    await invoke(async () => {
      await props.onAssignRole(props.targetUserIdInput, roleId);
    });
  };

  const onUnassignRole = async (): Promise<void> => {
    const roleId = assignmentRoleId();
    if (!roleId || !props.canManageMemberRoles || !props.hasActiveWorkspace) {
      return;
    }
    await invoke(async () => {
      await props.onUnassignRole(props.targetUserIdInput, roleId);
    });
  };

  return (
    <section class="member-group">
      <div class="button-row">
        <button
          type="button"
          onClick={() => void props.onRefreshRoles()}
          disabled={props.isLoadingRoles || !props.hasActiveWorkspace}
        >
          {props.isLoadingRoles ? "Refreshing..." : "Refresh roles"}
        </button>
        <button type="button" onClick={props.onOpenModerationPanel}>
          Open moderation panel
        </button>
      </div>

      <Show when={props.hasActiveWorkspace} fallback={<p class="muted">Select a workspace first.</p>}>
        <>
          <section class={roleHierarchyClass} aria-label="role hierarchy">
            <For each={hierarchyRoles()}>
              {(role) => (
                <button
                  type="button"
                  classList={{
                    [roleHierarchyItemClass]: true,
                    "border-brand": selectedRoleId() === role.roleId,
                    "opacity-90": role.isSystem,
                  }}
                  onClick={() => setSelectedRoleId(role.roleId)}
                >
                  <span>{role.name}</span>
                  <span class={roleHierarchyMetaClass}>position {role.position}</span>
                  <span class={roleHierarchyMetaClass}>{role.permissions.length} capabilities</span>
                  <Show when={role.isSystem}>
                    <span class={statusChipClass}>system</span>
                  </Show>
                </button>
              )}
            </For>
            <Show when={hierarchyRoles().length === 0}>
              <p class="muted">No roles available.</p>
            </Show>
          </section>

          <Show when={props.canManageWorkspaceRoles}>
            <form class="inline-form" onSubmit={onCreateRole}>
              <h5>Create Role</h5>
              <label>
                Role name
                <input
                  value={createName()}
                  onInput={(event) => setCreateName(event.currentTarget.value)}
                  maxlength="32"
                />
              </label>
              <div class={permissionGridClass} aria-label="create role permission matrix">
                <For each={PERMISSION_MATRIX}>
                  {(entry) => (
                    <label class={permissionToggleClass}>
                      <input
                        type="checkbox"
                        class="mt-[0.14rem]"
                        checked={createPermissions().includes(entry.permission)}
                        onChange={(event) =>
                          setCreatePermissions((current) =>
                            togglePermission(
                              current,
                              entry.permission,
                              event.currentTarget.checked,
                            ))}
                      />
                      <span class="text-[0.86rem] text-ink-1">{entry.label}</span>
                      <small class="col-[2] text-[0.74rem] text-ink-2">{entry.summary}</small>
                    </label>
                  )}
                </For>
              </div>
              <p class={rolePreviewClass}>
                Capability preview ({createPreview().length}):{" "}
                {createPreview()
                  .map((entry) => entry.permission)
                  .join(", ") || "none"}
              </p>
              <button type="submit" disabled={props.isMutatingRoles}>
                {props.isMutatingRoles ? "Applying..." : "Create role"}
              </button>
            </form>

            <form class="inline-form" onSubmit={onSaveRole}>
              <h5>Edit Selected Role</h5>
              <Show when={selectedRole()} fallback={<p class="muted">Select a role to edit.</p>}>
                {(roleAccessor) => (
                  <>
                    <label>
                      Role name
                      <input
                        value={editName()}
                        onInput={(event) => setEditName(event.currentTarget.value)}
                        maxlength="32"
                        disabled={roleAccessor().isSystem}
                      />
                    </label>
                    <div class={permissionGridClass} aria-label="edit role permission matrix">
                      <For each={PERMISSION_MATRIX}>
                        {(entry) => (
                          <label class={permissionToggleClass}>
                            <input
                              type="checkbox"
                              class="mt-[0.14rem]"
                              checked={editPermissions().includes(entry.permission)}
                              onChange={(event) =>
                                setEditPermissions((current) =>
                                  togglePermission(
                                    current,
                                    entry.permission,
                                    event.currentTarget.checked,
                                  ))}
                              disabled={roleAccessor().isSystem}
                            />
                            <span class="text-[0.86rem] text-ink-1">{entry.label}</span>
                            <small class="col-[2] text-[0.74rem] text-ink-2">{entry.summary}</small>
                          </label>
                        )}
                      </For>
                    </div>
                    <p class={rolePreviewClass}>
                      Capability preview ({editPreview().length}):{" "}
                      {editPreview()
                        .map((entry) => entry.permission)
                        .join(", ") || "none"}
                    </p>
                    <Show when={isRiskyPermissionDrop() && !roleAccessor().isSystem}>
                      <label class={checkboxRowClass}>
                        <input
                          type="checkbox"
                          checked={confirmRiskyRoleEdit()}
                          onChange={(event) =>
                            setConfirmRiskyRoleEdit(event.currentTarget.checked)}
                        />
                        <span>
                          I understand this may remove role-management capability from my operator
                          path.
                        </span>
                      </label>
                    </Show>
                    <div class="button-row">
                      <button
                        type="submit"
                        disabled={props.isMutatingRoles || roleAccessor().isSystem}
                      >
                        {props.isMutatingRoles ? "Applying..." : "Save role"}
                      </button>
                      <button
                        type="button"
                        disabled={props.isMutatingRoles || roleAccessor().isSystem}
                        onClick={() => void onDeleteRole()}
                      >
                        Delete role
                      </button>
                    </div>
                    <Show when={roleAccessor().isSystem}>
                      <p class="muted">
                        System roles are locked and cannot be edited or deleted from workspace UI.
                      </p>
                    </Show>
                  </>
                )}
              </Show>
            </form>

            <section class="inline-form">
              <h5>Role Hierarchy Reorder</h5>
              <For each={reorderDraftRoleIds()}>
                {(roleId, indexAccessor) => (
                  <div class="button-row items-center [&>span]:min-w-0 [&>span]:flex-1 [&>span]:break-words">
                    <span>
                      {hierarchyRoles().find((entry) => entry.roleId === roleId)?.name ??
                        "unknown"}
                    </span>
                    <button
                      type="button"
                      onClick={() => moveRole(roleId, "up")}
                      disabled={indexAccessor() === 0 || props.isMutatingRoles}
                    >
                      Up
                    </button>
                    <button
                      type="button"
                      onClick={() => moveRole(roleId, "down")}
                      disabled={
                        indexAccessor() === reorderDraftRoleIds().length - 1 ||
                        props.isMutatingRoles
                      }
                    >
                      Down
                    </button>
                  </div>
                )}
              </For>
              <button
                type="button"
                disabled={props.isMutatingRoles || reorderDraftRoleIds().length === 0}
                onClick={() => void onSaveReorder()}
              >
                Save hierarchy order
              </button>
            </section>
          </Show>

          <Show when={props.canManageMemberRoles}>
            <form class="inline-form" onSubmit={(event) => event.preventDefault()}>
              <h5>Member Role Assignment</h5>
              <label>
                Target user ULID
                <input
                  value={props.targetUserIdInput}
                  onInput={(event) => props.onTargetUserIdInput(event.currentTarget.value)}
                  maxlength="26"
                  placeholder="01ARZ..."
                />
              </label>
              <label>
                Role
                <select
                  value={assignmentRoleId() ?? ""}
                  onChange={(event) =>
                    setAssignmentRoleId(event.currentTarget.value as WorkspaceRoleId)}
                >
                  <For each={assignableRoles()}>
                    {(role) => <option value={role.roleId}>{role.name}</option>}
                  </For>
                </select>
              </label>
              <div class="button-row">
                <button
                  type="button"
                  disabled={props.isMutatingRoles || !assignmentRoleId()}
                  onClick={() => void onAssignRole()}
                >
                  Assign role
                </button>
                <button
                  type="button"
                  disabled={props.isMutatingRoles || !assignmentRoleId()}
                  onClick={() => void onUnassignRole()}
                >
                  Unassign role
                </button>
              </div>
              <p class="muted">
                Workspace owner promotion is server-owner-only and intentionally hidden here.
              </p>
            </form>
          </Show>
        </>
      </Show>

      <Show when={props.roleManagementStatus}>
        <p class={`${statusBaseClass} text-ok`}>{props.roleManagementStatus}</p>
      </Show>
      <Show when={props.roleManagementError || clientError()}>
        <p class={`${statusBaseClass} text-danger`}>{props.roleManagementError || clientError()}</p>
      </Show>
    </section>
  );
}
