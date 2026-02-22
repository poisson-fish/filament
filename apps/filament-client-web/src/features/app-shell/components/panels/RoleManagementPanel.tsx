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
  category: "workspace" | "moderation" | "voice" | "compatibility";
}

const PERMISSION_MATRIX: PermissionMatrixEntry[] = [
  {
    permission: "create_message",
    label: "Create Messages",
    summary: "Send messages and participate in channels.",
    category: "workspace",
  },
  {
    permission: "delete_message",
    label: "Delete Messages",
    summary: "Delete or edit messages authored by other members.",
    category: "moderation",
  },
  {
    permission: "manage_channel_overrides",
    label: "Manage Overrides",
    summary: "Edit channel role override rules.",
    category: "workspace",
  },
  {
    permission: "ban_member",
    label: "Ban Members",
    summary: "Kick and ban users at workspace scope.",
    category: "moderation",
  },
  {
    permission: "manage_member_roles",
    label: "Manage Member Roles",
    summary: "Assign and unassign workspace roles on members.",
    category: "workspace",
  },
  {
    permission: "manage_workspace_roles",
    label: "Manage Workspace Roles",
    summary: "Create, update, delete, and reorder workspace roles.",
    category: "workspace",
  },
  {
    permission: "view_audit_log",
    label: "View Audit Log",
    summary: "Read redacted workspace audit history.",
    category: "moderation",
  },
  {
    permission: "manage_ip_bans",
    label: "Manage IP Bans",
    summary: "Apply and remove user-derived guild IP bans.",
    category: "moderation",
  },
  {
    permission: "publish_video",
    label: "Publish Camera",
    summary: "Publish camera tracks in voice channels.",
    category: "voice",
  },
  {
    permission: "publish_screen_share",
    label: "Publish Screen",
    summary: "Publish screen-share tracks in voice channels.",
    category: "voice",
  },
  {
    permission: "subscribe_streams",
    label: "Subscribe Streams",
    summary: "Receive remote media streams in voice channels.",
    category: "voice",
  },
  {
    permission: "manage_roles",
    label: "Legacy Manage Roles",
    summary: "Compatibility grant for pre-phase-7 moderation paths.",
    category: "compatibility",
  },
];

interface PermissionCategory {
  key: PermissionMatrixEntry["category"];
  title: string;
}

const PERMISSION_CATEGORIES: PermissionCategory[] = [
  { key: "workspace", title: "Workspace Access" },
  { key: "moderation", title: "Moderation" },
  { key: "voice", title: "Voice & Media" },
  { key: "compatibility", title: "Compatibility" },
];

type RoleTemplateKey = "custom" | "cosmetic" | "moderator" | "read_only";

interface RoleTemplate {
  key: RoleTemplateKey;
  label: string;
  summary: string;
  defaultName: string;
  defaultPermissions: PermissionName[];
}

const ROLE_TEMPLATES: RoleTemplate[] = [
  {
    key: "custom",
    label: "Custom Role",
    summary: "Start from the baseline responder permissions.",
    defaultName: "Responder",
    defaultPermissions: ["create_message", "subscribe_streams"],
  },
  {
    key: "cosmetic",
    label: "Cosmetic Role",
    summary: "No extra capabilities; use for labels and presentation only.",
    defaultName: "Cosmetic",
    defaultPermissions: [],
  },
  {
    key: "moderator",
    label: "Moderator",
    summary: "Preloads common moderation capabilities for trusted operators.",
    defaultName: "Moderator",
    defaultPermissions: [
      "create_message",
      "subscribe_streams",
      "delete_message",
      "ban_member",
      "view_audit_log",
      "manage_ip_bans",
    ],
  },
  {
    key: "read_only",
    label: "Read-Only",
    summary: "Stream receive only; excludes message creation and media publishing.",
    defaultName: "Read-Only",
    defaultPermissions: ["subscribe_streams"],
  },
];

const ROLE_TEMPLATE_BY_KEY: Record<RoleTemplateKey, RoleTemplate> = {
  custom: ROLE_TEMPLATES[0]!,
  cosmetic: ROLE_TEMPLATES[1]!,
  moderator: ROLE_TEMPLATES[2]!,
  read_only: ROLE_TEMPLATES[3]!,
};

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

function normalizePermissions(permissions: ReadonlyArray<PermissionName>): PermissionName[] {
  const set = new Set<PermissionName>();
  for (const permission of permissions) {
    set.add(permission);
  }
  return [...set].sort((left, right) => left.localeCompare(right));
}

function normalizePermissionsByMatrix(
  permissions: ReadonlyArray<PermissionName>,
): PermissionName[] {
  const allowed = new Set(PERMISSION_MATRIX.map((entry) => entry.permission));
  const selected = new Set(permissions.filter((permission) => allowed.has(permission)));
  const normalized: PermissionName[] = [];
  for (const entry of PERMISSION_MATRIX) {
    if (selected.has(entry.permission)) {
      normalized.push(entry.permission);
    }
  }
  return normalized;
}

function areSamePermissions(
  left: ReadonlyArray<PermissionName>,
  right: ReadonlyArray<PermissionName>,
): boolean {
  const normalizedLeft = normalizePermissions(left);
  const normalizedRight = normalizePermissions(right);
  if (normalizedLeft.length !== normalizedRight.length) {
    return false;
  }
  return normalizedLeft.every((permission, index) => permission === normalizedRight[index]);
}

function permissionsByCategory(
  category: PermissionMatrixEntry["category"],
): PermissionMatrixEntry[] {
  return PERMISSION_MATRIX.filter((entry) => entry.category === category);
}

export function RoleManagementPanel(props: RoleManagementPanelProps) {
  const [clientError, setClientError] = createSignal("");

  const [createTemplateKey, setCreateTemplateKey] = createSignal<RoleTemplateKey>("custom");
  const [createName, setCreateName] = createSignal(ROLE_TEMPLATE_BY_KEY.custom.defaultName);
  const [createPermissions, setCreatePermissions] = createSignal<PermissionName[]>(
    ROLE_TEMPLATE_BY_KEY.custom.defaultPermissions,
  );

  const [selectedRoleId, setSelectedRoleId] = createSignal<WorkspaceRoleId | null>(null);
  const [editName, setEditName] = createSignal("");
  const [editPermissions, setEditPermissions] = createSignal<PermissionName[]>([]);
  const [confirmRiskyRoleEdit, setConfirmRiskyRoleEdit] = createSignal(false);

  const [reorderDraftRoleIds, setReorderDraftRoleIds] = createSignal<WorkspaceRoleId[]>([]);
  const [draggingRoleId, setDraggingRoleId] = createSignal<WorkspaceRoleId | null>(null);
  const [dragOverRoleId, setDragOverRoleId] = createSignal<WorkspaceRoleId | null>(null);

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
  const panelSectionClass = "grid gap-[0.5rem]";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const actionButtonClass =
    "min-h-[1.95rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const actionButtonRowClass = "flex gap-[0.45rem]";
  const rowActionButtonClass = `${actionButtonClass} flex-1`;
  const mutedTextClass = "m-0 text-[0.91rem] text-ink-2";
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
  const createRoleTemplate = createMemo(() => ROLE_TEMPLATE_BY_KEY[createTemplateKey()]);
  const editPreview = createMemo(() =>
    PERMISSION_MATRIX.filter((entry) => editPermissions().includes(entry.permission)),
  );
  const isCreateRoleNameValid = createMemo(() => createName().trim().length > 0);
  const hasRoleDraftChanges = createMemo(() => {
    const role = editableRole();
    if (!role) {
      return false;
    }
    if (editName().trim() !== role.name) {
      return true;
    }
    return !areSamePermissions(editPermissions(), role.permissions);
  });
  const isRoleNameValid = createMemo(() => editName().trim().length > 0);

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
    setDraggingRoleId(null);
    setDragOverRoleId(null);
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
    if (!isCreateRoleNameValid()) {
      setClientError("Role name must include visible characters.");
      return;
    }

    await invoke(async () => {
      await props.onCreateRole({
        name: createName().trim(),
        permissions: normalizePermissionsByMatrix(createPermissions()),
      });
      setCreateTemplateKey("custom");
      setCreateName(ROLE_TEMPLATE_BY_KEY.custom.defaultName);
      setCreatePermissions(ROLE_TEMPLATE_BY_KEY.custom.defaultPermissions);
    });
  };

  const onCreateRoleTemplateChange = (templateKey: RoleTemplateKey): void => {
    const template = ROLE_TEMPLATE_BY_KEY[templateKey];
    setCreateTemplateKey(templateKey);
    setCreateName(template.defaultName);
    setCreatePermissions(normalizePermissionsByMatrix(template.defaultPermissions));
    setClientError("");
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
    if (!hasRoleDraftChanges()) {
      setClientError("No role changes to save.");
      return;
    }
    if (!isRoleNameValid()) {
      setClientError("Role name must include visible characters.");
      return;
    }

    await invoke(async () => {
      const nextName = editName().trim();
      const normalizedPermissions = normalizePermissions(editPermissions());
      const roleNameChanged = nextName !== role.name;
      const permissionsChanged = !areSamePermissions(normalizedPermissions, role.permissions);
      const updateInput: {
        name?: string;
        permissions?: PermissionName[];
      } = {};
      if (roleNameChanged) {
        updateInput.name = nextName;
      }
      if (permissionsChanged) {
        updateInput.permissions = normalizedPermissions;
      }
      await props.onUpdateRole(role.roleId, updateInput);
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

  const moveRoleToTarget = (roleId: WorkspaceRoleId, targetRoleId: WorkspaceRoleId): void => {
    setReorderDraftRoleIds((current) => {
      const sourceIndex = current.findIndex((value) => value === roleId);
      const targetIndex = current.findIndex((value) => value === targetRoleId);
      if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex) {
        return current;
      }

      const next = [...current];
      const [moved] = next.splice(sourceIndex, 1);
      next.splice(targetIndex, 0, moved!);
      return next;
    });
  };

  const onReorderDragStart = (roleId: WorkspaceRoleId): void => {
    if (!props.canManageWorkspaceRoles || props.isMutatingRoles) {
      return;
    }
    setDraggingRoleId(roleId);
    setDragOverRoleId(roleId);
  };

  const onReorderDrop = (targetRoleId: WorkspaceRoleId): void => {
    const sourceRoleId = draggingRoleId();
    if (!sourceRoleId || !props.canManageWorkspaceRoles || props.isMutatingRoles) {
      setDragOverRoleId(null);
      return;
    }
    moveRoleToTarget(sourceRoleId, targetRoleId);
    setDragOverRoleId(null);
    setDraggingRoleId(null);
  };

  const onReorderDragEnd = (): void => {
    setDraggingRoleId(null);
    setDragOverRoleId(null);
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

  const onResetRoleDraft = (): void => {
    const role = selectedRole();
    if (!role) {
      return;
    }
    setEditName(role.name);
    setEditPermissions(role.permissions);
    setConfirmRiskyRoleEdit(false);
    setClientError("");
  };

  return (
    <section class={panelSectionClass}>
      <div class={actionButtonRowClass}>
        <button
          class={rowActionButtonClass}
          type="button"
          onClick={() => void props.onRefreshRoles()}
          disabled={props.isLoadingRoles || !props.hasActiveWorkspace}
        >
          {props.isLoadingRoles ? "Refreshing..." : "Refresh roles"}
        </button>
        <button class={rowActionButtonClass} type="button" onClick={props.onOpenModerationPanel}>
          Open moderation panel
        </button>
      </div>

      <Show when={props.hasActiveWorkspace} fallback={<p class={mutedTextClass}>Select a workspace first.</p>}>
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
              <p class={mutedTextClass}>No roles available.</p>
            </Show>
          </section>

          <Show when={props.canManageWorkspaceRoles}>
            <form class={formClass} onSubmit={onCreateRole}>
              <h5>Create Role</h5>
              <label class={fieldLabelClass}>
                Template
                <select
                  class={fieldControlClass}
                  value={createTemplateKey()}
                  onChange={(event) =>
                    onCreateRoleTemplateChange(
                      event.currentTarget.value as RoleTemplateKey,
                    )}
                  aria-label="Role template"
                >
                  <For each={ROLE_TEMPLATES}>
                    {(template) => (
                      <option value={template.key}>{template.label}</option>
                    )}
                  </For>
                </select>
              </label>
              <p class={mutedTextClass}>{createRoleTemplate().summary}</p>
              <label class={fieldLabelClass}>
                Role name
                <input
                  class={fieldControlClass}
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
              <Show when={!isCreateRoleNameValid()}>
                <p class={mutedTextClass}>Role name must include visible characters.</p>
              </Show>
              <button
                class={actionButtonClass}
                type="submit"
                disabled={props.isMutatingRoles || !isCreateRoleNameValid()}
              >
                {props.isMutatingRoles ? "Applying..." : "Create role"}
              </button>
            </form>

            <form class={formClass} onSubmit={onSaveRole}>
              <h5>Edit Selected Role</h5>
              <Show when={selectedRole()} fallback={<p class={mutedTextClass}>Select a role to edit.</p>}>
                {(roleAccessor) => (
                  <>
                    <Show when={hasRoleDraftChanges() && !roleAccessor().isSystem}>
                      <p class={statusChipClass}>unsaved changes</p>
                    </Show>
                    <label class={fieldLabelClass}>
                      Role name
                      <input
                        class={fieldControlClass}
                        value={editName()}
                        onInput={(event) => setEditName(event.currentTarget.value)}
                        maxlength="32"
                        disabled={roleAccessor().isSystem}
                      />
                    </label>
                    <div class={permissionGridClass} aria-label="edit role permission matrix">
                      <For each={PERMISSION_CATEGORIES}>
                        {(category) => (
                          <section class={panelSectionClass}>
                            <h6 class="m-0 text-[0.82rem] uppercase tracking-[0.06em] text-ink-2">
                              {category.title}
                            </h6>
                            <For each={permissionsByCategory(category.key)}>
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
                          </section>
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
                    <div class={actionButtonRowClass}>
                      <button
                        class={rowActionButtonClass}
                        type="submit"
                        disabled={
                          props.isMutatingRoles ||
                          roleAccessor().isSystem ||
                          !hasRoleDraftChanges() ||
                          !isRoleNameValid()
                        }
                      >
                        {props.isMutatingRoles ? "Applying..." : "Save role"}
                      </button>
                      <button
                        class={rowActionButtonClass}
                        type="button"
                        disabled={props.isMutatingRoles || roleAccessor().isSystem || !hasRoleDraftChanges()}
                        onClick={onResetRoleDraft}
                      >
                        Reset draft
                      </button>
                      <button
                        class={rowActionButtonClass}
                        type="button"
                        disabled={props.isMutatingRoles || roleAccessor().isSystem}
                        onClick={() => void onDeleteRole()}
                      >
                        Delete role
                      </button>
                    </div>
                    <Show when={roleAccessor().isSystem}>
                      <p class={mutedTextClass}>
                        System roles are locked and cannot be edited or deleted from workspace UI.
                      </p>
                    </Show>
                  </>
                )}
              </Show>
            </form>

            <section class={formClass}>
              <h5>Role Hierarchy Reorder</h5>
              <p class={mutedTextClass}>
                Drag custom roles to reorder hierarchy. System roles stay pinned.
              </p>
              <For each={reorderDraftRoleIds()}>
                {(roleId) => (
                  <div
                    classList={{
                      [`${actionButtonRowClass} items-center [&>span]:min-w-0 [&>span]:flex-1 [&>span]:break-words rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.48rem] py-[0.36rem]`]:
                        true,
                      "border-brand": dragOverRoleId() === roleId,
                    }}
                    aria-label={`Reorder role ${hierarchyRoles().find((entry) => entry.roleId === roleId)?.name ?? "unknown"}`}
                    draggable={props.canManageWorkspaceRoles && !props.isMutatingRoles}
                    onDragStart={() => onReorderDragStart(roleId)}
                    onDragOver={(event) => {
                      event.preventDefault();
                      if (!props.canManageWorkspaceRoles || props.isMutatingRoles) {
                        return;
                      }
                      if (dragOverRoleId() !== roleId) {
                        setDragOverRoleId(roleId);
                      }
                    }}
                    onDrop={(event) => {
                      event.preventDefault();
                      onReorderDrop(roleId);
                    }}
                    onDragEnd={onReorderDragEnd}
                  >
                    <span>
                      {hierarchyRoles().find((entry) => entry.roleId === roleId)?.name ??
                        "unknown"}
                    </span>
                    <span class={statusChipClass}>drag</span>
                  </div>
                )}
              </For>
              <button
                class={actionButtonClass}
                type="button"
                disabled={props.isMutatingRoles || reorderDraftRoleIds().length === 0}
                onClick={() => void onSaveReorder()}
              >
                Save hierarchy order
              </button>
            </section>
          </Show>

          <Show when={props.canManageMemberRoles}>
            <form class={formClass} onSubmit={(event) => event.preventDefault()}>
              <h5>Member Role Assignment</h5>
              <label class={fieldLabelClass}>
                Target user ULID
                <input
                  class={fieldControlClass}
                  value={props.targetUserIdInput}
                  onInput={(event) => props.onTargetUserIdInput(event.currentTarget.value)}
                  maxlength="26"
                  placeholder="01ARZ..."
                />
              </label>
              <label class={fieldLabelClass}>
                Role
                <select
                  class={fieldControlClass}
                  value={assignmentRoleId() ?? ""}
                  onChange={(event) =>
                    setAssignmentRoleId(event.currentTarget.value as WorkspaceRoleId)}
                >
                  <For each={assignableRoles()}>
                    {(role) => <option value={role.roleId}>{role.name}</option>}
                  </For>
                </select>
              </label>
              <div class={actionButtonRowClass}>
                <button
                  class={rowActionButtonClass}
                  type="button"
                  disabled={props.isMutatingRoles || !assignmentRoleId()}
                  onClick={() => void onAssignRole()}
                >
                  Assign role
                </button>
                <button
                  class={rowActionButtonClass}
                  type="button"
                  disabled={props.isMutatingRoles || !assignmentRoleId()}
                  onClick={() => void onUnassignRole()}
                >
                  Unassign role
                </button>
              </div>
              <p class={mutedTextClass}>
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
