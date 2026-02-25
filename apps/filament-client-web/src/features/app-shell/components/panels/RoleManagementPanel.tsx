import { For, Match, Show, Switch, createEffect, createMemo, createSignal } from "solid-js";
import {
  type PermissionName,
  type GuildRoleRecord,
  type WorkspaceRoleId,
  workspaceRoleIdFromInput,
} from "../../../../domain/chat";
import {
  PERMISSION_CATEGORIES,
  PERMISSION_MATRIX,
  type PermissionMatrixEntry,
} from "../../permissions/permission-metadata";

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

type DangerOperation = "save_role" | "delete_role" | "reorder_roles";

interface DangerModalState {
  operation: DangerOperation;
  title: string;
  message: string;
  confirmLabel: string;
}

export interface RoleManagementPanelProps {
  hasActiveWorkspace: boolean;
  canManageWorkspaceRoles: boolean;
  canManageMemberRoles: boolean;
  roles: GuildRoleRecord[];
  isLoadingRoles: boolean;
  isMutatingRoles: boolean;
  roleManagementStatus: string;
  roleManagementError: string;
  defaultJoinRoleId?: WorkspaceRoleId | null;
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
  onUpdateDefaultJoinRole?: (roleId: WorkspaceRoleId | null) => Promise<void> | void;
  onOpenModerationPanel?: () => void;
  embedded?: boolean;
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

function hasPrivilegedPermission(permissions: ReadonlyArray<PermissionName>): boolean {
  return permissions.some(
    (permission) =>
      permission === "manage_workspace_roles" ||
      permission === "manage_member_roles" ||
      permission === "manage_roles" ||
      permission === "ban_member" ||
      permission === "manage_ip_bans" ||
      permission === "manage_channel_overrides",
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
  const [dangerModal, setDangerModal] = createSignal<DangerModalState | null>(null);

  const [reorderDraftRoleIds, setReorderDraftRoleIds] = createSignal<WorkspaceRoleId[]>([]);
  const [draggingRoleId, setDraggingRoleId] = createSignal<WorkspaceRoleId | null>(null);
  const [dragOverRoleId, setDragOverRoleId] = createSignal<WorkspaceRoleId | null>(null);

  const [assignmentRoleId, setAssignmentRoleId] = createSignal<WorkspaceRoleId | null>(null);

  // Layout states
  const [isCreatingRole, setIsCreatingRole] = createSignal(false);
  const [activeTab, setActiveTab] = createSignal<"display" | "permissions" | "members">("display");

  // CSS classes
  const panelSectionClass = props.embedded ? "flex flex-col gap-[0.66rem] h-full" : "flex flex-col gap-[0.5rem] h-full";
  const formClass = "grid gap-[1rem]";
  const fieldLabelClass = "grid gap-[0.4rem] text-[0.84rem] text-ink-1 uppercase tracking-wide font-semibold";
  const fieldControlClass =
    "rounded-[0.4rem] border border-line-soft bg-bg-0 px-[0.7rem] py-[0.62rem] text-[0.92rem] text-ink-0 focus:border-brand focus:outline-none focus:ring-1 focus:ring-brand disabled:cursor-default disabled:opacity-62 transition-shadow";
  const actionButtonClass =
    "min-h-[2.2rem] rounded-[0.4rem] border border-line-soft bg-bg-2 px-[1rem] py-[0.5rem] text-[0.92rem] font-medium text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3 enabled:hover:text-ink-0 disabled:cursor-default disabled:opacity-50";
  const primaryButtonClass = "min-h-[2.2rem] rounded-[0.4rem] bg-ink-0 px-[1rem] py-[0.5rem] text-[0.92rem] font-medium text-bg-0 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-ink-1 disabled:cursor-default disabled:opacity-50";
  const toolbarButtonClass =
    "min-h-[1.8rem] rounded-[0.4rem] border border-line-soft bg-bg-2 px-[0.6rem] py-[0.3rem] text-[0.75rem] font-medium text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3 enabled:hover:text-ink-0 disabled:cursor-default disabled:opacity-60";
  const statusChipClass =
    "inline-block rounded-full bg-bg-2 px-[0.4rem] py-[0.1rem] text-[0.65rem] uppercase tracking-[0.06em] text-ink-2";
  const mutedTextClass = "m-0 text-[0.88rem] text-ink-2 leading-relaxed";
  
  // Right pane tab styles
  const tabButtonClass = "appearance-none bg-transparent rounded-none border-0 border-b-[2px] px-[0.5rem] pb-[0.6rem] text-[0.96rem] font-medium transition-colors -mb-[1px] cursor-pointer";
  const tabButtonActive = "border-brand text-ink-0";
  const tabButtonInactive = "border-transparent text-ink-2 hover:text-ink-1 hover:border-line-soft";

  // Permission switch
  const permissionToggleClass = "flex items-center justify-between rounded-[0.5rem] border border-line-soft bg-bg-0 px-[1rem] py-[0.8rem] transition-colors hover:border-line";

  const hierarchyRoles = createMemo(() => sortRolesByHierarchy(props.roles));
  const assignableRoles = createMemo(() =>
    hierarchyRoles().filter((role) => !role.isSystem),
  );
  const defaultJoinRoleControlValue = createMemo(() => props.defaultJoinRoleId ?? "");

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

  const createRoleTemplate = createMemo(() => ROLE_TEMPLATE_BY_KEY[createTemplateKey()]);
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
  const isRiskyPermissionEscalation = createMemo(() => {
    const role = editableRole();
    if (!role) {
      return false;
    }
    return (
      !hasPrivilegedPermission(role.permissions) &&
      hasPrivilegedPermission(editPermissions())
    );
  });

  const hasReorderChanges = createMemo(() => {
    const original = assignableRoles().map((r) => r.roleId);
    const draft = reorderDraftRoleIds();
    if (original.length !== draft.length) return false;
    for (let i = 0; i < original.length; i++) {
      if (original[i] !== draft[i]) return true;
    }
    return false;
  });

  createEffect(() => {
    const roles = hierarchyRoles();
    if (roles.length === 0) {
      setSelectedRoleId(null);
      return;
    }
    const current = selectedRoleId();
    if (!current || !roles.some((entry) => entry.roleId === current)) {
      const firstCustomRole = roles.find((entry) => !entry.isSystem);
      setSelectedRoleId((firstCustomRole ?? roles[0])!.roleId);
    }
  });

  createEffect(() => {
    const role = selectedRole();
    if (!role) {
      setEditName("");
      setEditPermissions([]);
      return;
    }
    setEditName(role.name);
    setEditPermissions(role.permissions);
  });

  createEffect(() => {
    const reorderable = assignableRoles().map((role) => role.roleId);
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
      setIsCreatingRole(false);
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
    const needsDangerConfirmation =
      isRiskyPermissionDrop() || isRiskyPermissionEscalation();
    if (needsDangerConfirmation) {
      setDangerModal({
        operation: "save_role",
        title: "Confirm dangerous permission change",
        message: isRiskyPermissionDrop()
          ? "This update removes role-management capabilities and can lock operators out of moderation workflows."
          : "This update grants privileged permissions that can escalate workspace control if assigned broadly.",
        confirmLabel: "Apply dangerous change",
      });
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
    setDangerModal({
      operation: "delete_role",
      title: "Delete role?",
      message: `Role "${role.name}" will be permanently removed from this workspace. This cannot be undone.`,
      confirmLabel: "Delete role",
    });
  };
  const onDeleteRoleFromList = (roleId: WorkspaceRoleId): void => {
    if (!props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    const role = hierarchyRoles().find((entry) => entry.roleId === roleId);
    if (!role || role.isSystem) {
      return;
    }
    setIsCreatingRole(false);
    setSelectedRoleId(roleId);
    setDangerModal({
      operation: "delete_role",
      title: "Delete role?",
      message: `Role "${role.name}" will be permanently removed from this workspace. This cannot be undone.`,
      confirmLabel: "Delete role",
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
    setDangerModal({
      operation: "reorder_roles",
      title: "Apply hierarchy reorder?",
      message:
        "This updates role precedence across moderation and assignment checks. Review ordering before saving.",
      confirmLabel: "Save hierarchy order",
    });
  };

  const onCancelReorder = (): void => {
    const reorderable = assignableRoles().map((role) => role.roleId);
    setReorderDraftRoleIds(reorderable);
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

  const onDefaultJoinRoleChange = async (value: string): Promise<void> => {
    if (!props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    await invoke(async () => {
      await props.onUpdateDefaultJoinRole?.(
        value.length > 0 ? workspaceRoleIdFromInput(value) : null,
      );
    });
  };

  const onResetRoleDraft = (): void => {
    const role = selectedRole();
    if (!role) {
      return;
    }
    setEditName(role.name);
    setEditPermissions(role.permissions);
    setClientError("");
  };
  const onSaveRoleNameOnly = async (): Promise<void> => {
    const role = editableRole();
    if (!role || !props.canManageWorkspaceRoles || !props.hasActiveWorkspace) {
      return;
    }
    const nextName = editName().trim();
    if (!nextName) {
      setClientError("Role name must include visible characters.");
      return;
    }
    if (nextName === role.name) {
      setClientError("Role name is unchanged.");
      return;
    }
    await invoke(async () => {
      await props.onUpdateRole(role.roleId, { name: nextName });
    });
  };

  const onConfirmDangerModal = async (): Promise<void> => {
    const pending = dangerModal();
    if (!pending) {
      return;
    }
    setDangerModal(null);
    if (pending.operation === "save_role") {
      await invoke(async () => {
        const role = editableRole();
        if (!role) {
          return;
        }
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
      return;
    }
    if (pending.operation === "delete_role") {
      await invoke(async () => {
        const role = editableRole();
        if (!role) {
          return;
        }
        await props.onDeleteRole(role.roleId);
      });
      return;
    }
    if (pending.operation === "reorder_roles") {
      await invoke(async () => {
        await props.onReorderRoles(reorderDraftRoleIds());
      });
    }
  };

  const getSystemRoles = createMemo(() => hierarchyRoles().filter(r => r.isSystem));

  return (
    <section class={panelSectionClass}>
      <Show when={props.hasActiveWorkspace} fallback={<p class={mutedTextClass}>Select a workspace first.</p>}>
        <div class="flex justify-end gap-[0.5rem] mb-[0.5rem]">
          <button
            class={toolbarButtonClass}
            type="button"
            onClick={() => void props.onRefreshRoles()}
            disabled={props.isLoadingRoles || !props.hasActiveWorkspace}
          >
            {props.isLoadingRoles ? "Refreshing..." : "Refresh roles"}
          </button>
          <Show when={props.onOpenModerationPanel}>
            {(onOpenModerationPanel) => (
              <button
                class={toolbarButtonClass}
                type="button"
                onClick={() => onOpenModerationPanel()}
              >
                Moderation tools
              </button>
            )}
          </Show>
        </div>
        <div class="flex h-[80vh] min-h-[500px] w-full flex-col md:flex-row bg-bg-1 rounded-[0.5rem] overflow-hidden border border-line-soft">
          {/* Left Sidebar: Roles List */}
          <div class="flex flex-col w-full md:w-[280px] flex-shrink-0 bg-bg-1 border-r border-line-soft">
            <div class="flex items-center justify-between p-[1rem] border-b border-line-soft">
              <h3 class="m-0 text-[0.8rem] uppercase tracking-[0.05em] font-semibold text-ink-2">Roles</h3>
              <Show when={props.canManageWorkspaceRoles}>
                <button
                  type="button"
                  class={`${toolbarButtonClass} flex items-center gap-[0.3rem]`}
                  onClick={() => setIsCreatingRole(true)}
                  disabled={props.isMutatingRoles}
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                  </svg>
                  Create role
                </button>
              </Show>
            </div>

            <div class="flex-1 overflow-y-auto p-[0.5rem] flex flex-col gap-[0.2rem]">
              <Show when={getSystemRoles().length > 0}>
                <div class="px-[0.5rem] py-[0.4rem] mt-[0.4rem]">
                  <span class="text-[0.7rem] uppercase tracking-wider text-ink-3 font-semibold">System</span>
                </div>
                <For each={getSystemRoles()}>
                  {(role) => (
                    <button
                      type="button"
                      classList={{
                        "flex items-center justify-between gap-[0.5rem] px-[0.6rem] py-[0.5rem] rounded-[0.4rem] text-left transition-colors border border-transparent bg-transparent": true,
                        "bg-bg-3 text-ink-0": !isCreatingRole() && selectedRoleId() === role.roleId,
                        "hover:bg-bg-2 text-ink-1 opacity-80": isCreatingRole() || selectedRoleId() !== role.roleId,
                      }}
                      onClick={() => { setIsCreatingRole(false); setSelectedRoleId(role.roleId); }}
                    >
                      <span class="truncate font-medium flex-1">{role.name}</span>
                      <svg class="w-3.5 h-3.5 text-ink-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                        <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                      </svg>
                    </button>
                  )}
                </For>
              </Show>

              <div class="px-[0.5rem] py-[0.4rem] mt-[0.8rem]">
                <span class="text-[0.7rem] uppercase tracking-wider text-ink-3 font-semibold">Custom Roles</span>
              </div>
              <div class="px-[0.5rem] pb-[0.7rem]">
                <label class="grid gap-[0.35rem] text-[0.72rem] uppercase tracking-wider text-ink-3 font-semibold">
                  Default Join Role
                  <select
                    class={fieldControlClass}
                    value={defaultJoinRoleControlValue()}
                    onChange={(event) => {
                      void onDefaultJoinRoleChange(event.currentTarget.value);
                    }}
                    disabled={!props.canManageWorkspaceRoles || props.isMutatingRoles}
                  >
                    <option value="">No default custom role</option>
                    <For each={assignableRoles()}>
                      {(role) => <option value={role.roleId}>{role.name}</option>}
                    </For>
                  </select>
                </label>
              </div>
              
              <div class="flex flex-col gap-[0.2rem]">
                <For each={reorderDraftRoleIds()}>
                  {(roleId) => {
                    const role = hierarchyRoles().find(r => r.roleId === roleId);
                    if (!role) return null;
                    return (
                      <div
                        classList={{
                          "flex items-center gap-[0.3rem] rounded-[0.4rem] transition-colors cursor-pointer select-none border border-transparent bg-transparent px-[0.25rem]": true,
                          "bg-bg-3 text-ink-0 border-transparent": !isCreatingRole() && selectedRoleId() === role.roleId && dragOverRoleId() !== roleId,
                          "hover:bg-bg-2 text-ink-1 border-transparent": (isCreatingRole() || selectedRoleId() !== role.roleId) && dragOverRoleId() !== roleId,
                          "border-brand bg-bg-2": dragOverRoleId() === roleId,
                        }}
                        draggable={props.canManageWorkspaceRoles && !props.isMutatingRoles}
                        onDragStart={() => onReorderDragStart(roleId)}
                        onDragOver={(event) => {
                          event.preventDefault();
                          if (!props.canManageWorkspaceRoles || props.isMutatingRoles) return;
                          if (dragOverRoleId() !== roleId) setDragOverRoleId(roleId);
                        }}
                        onDrop={(event) => {
                          event.preventDefault();
                          onReorderDrop(roleId);
                        }}
                        onDragEnd={onReorderDragEnd}
                      >
                        <button
                          type="button"
                          class="min-w-0 flex-1 truncate px-[0.35rem] py-[0.5rem] text-left font-medium text-inherit bg-transparent border-0"
                          onClick={() => { setIsCreatingRole(false); setSelectedRoleId(role.roleId); }}
                        >
                          {role.name}
                        </button>
                        <div class="flex items-center gap-[0.2rem]">
                          <button
                            type="button"
                            class="inline-flex h-[1.45rem] w-[1.45rem] items-center justify-center rounded-[0.35rem] border border-danger bg-danger text-danger-ink transition-colors hover:brightness-110 disabled:cursor-default disabled:opacity-60"
                            aria-label={`Delete role ${role.name}`}
                            title={
                              props.canManageWorkspaceRoles
                                ? `Delete role ${role.name}`
                                : "You do not have permission to delete roles."
                            }
                            disabled={props.isMutatingRoles || !props.canManageWorkspaceRoles}
                            onClick={(event) => {
                              event.stopPropagation();
                              void onDeleteRoleFromList(role.roleId);
                            }}
                          >
                            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                              <polyline points="3 6 5 6 21 6" />
                              <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                              <path d="M10 11v6" />
                              <path d="M14 11v6" />
                              <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
                            </svg>
                          </button>
                          <Show when={props.canManageWorkspaceRoles}>
                            <div class="cursor-grab active:cursor-grabbing p-[0.2rem] text-ink-3 hover:text-ink-1" aria-label="Drag to reorder">
                              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <line x1="8" y1="6" x2="21" y2="6" />
                                <line x1="8" y1="12" x2="21" y2="12" />
                              <line x1="8" y1="18" x2="21" y2="18" />
                              <line x1="3" y1="6" x2="3.01" y2="6" />
                              <line x1="3" y1="12" x2="3.01" y2="12" />
                                <line x1="3" y1="18" x2="3.01" y2="18" />
                              </svg>
                            </div>
                          </Show>
                        </div>
                      </div>
                    );
                  }}
                </For>
                <Show when={reorderDraftRoleIds().length === 0}>
                   <p class="px-[0.6rem] py-[0.5rem] text-[0.85rem] text-ink-3 italic">No custom roles yet.</p>
                </Show>
              </div>

              <Show when={hasReorderChanges()}>
                <div class="mt-[1rem] p-[0.8rem] bg-bg-2 rounded-[0.4rem] border border-line-soft flex flex-col gap-[0.5rem]">
                  <p class="m-0 text-[0.8rem] text-ink-1 text-center font-medium">Careful - you have unsaved changes!</p>
                  <div class="flex gap-[0.4rem]">
                    <button class="flex-1 text-[0.8rem] py-[0.3rem] rounded bg-ink-2 text-bg-0 font-medium hover:bg-ink-1" onClick={onCancelReorder}>Reset</button>
                    <button class="flex-1 text-[0.8rem] py-[0.3rem] rounded bg-ok text-ok-ink font-medium hover:brightness-110" onClick={() => void onSaveReorder()} disabled={props.isMutatingRoles}>Save</button>
                  </div>
                </div>
              </Show>
            </div>
          </div>

          {/* Right Pane: Main Content Area */}
          <div class="flex-1 bg-bg-1 overflow-y-auto relative">
            <Show when={isCreatingRole()}>
              {/* Create Role View */}
              <div class="p-[2rem] max-w-[800px] mx-auto pb-[6rem]">
                <header class="mb-[2rem]">
                  <h2 class="m-0 text-[1.4rem] font-semibold text-ink-0">Create Role</h2>
                  <p class={mutedTextClass}>Create a new custom role to assign to members.</p>
                </header>

                <form class={formClass} onSubmit={onCreateRole}>
                  <section class="grid gap-[1rem] p-[1.5rem] bg-bg-2 border border-line-soft rounded-[0.6rem] max-w-[500px]">
                    <label class={fieldLabelClass}>
                      Role name
                      <input
                        class={fieldControlClass}
                        value={createName()}
                        onInput={(event) => setCreateName(event.currentTarget.value)}
                        maxlength="32"
                        placeholder="e.g. VIP, Moderator, Member"
                        autofocus
                      />
                    </label>

                    <label class={fieldLabelClass}>
                      Start from Template
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
                      <span class="text-[0.8rem] text-ink-2 normal-case font-normal mt-[0.2rem]">{createRoleTemplate().summary}</span>
                    </label>
                  </section>

                  <section class="mt-[1rem]">
                    <div class="flex items-center justify-between mb-[1rem]">
                      <h4 class="m-0 text-[1rem] font-medium text-ink-0">Permissions</h4>
                      <span class="text-[0.8rem] text-ink-2">{createPermissions().length} enabled</span>
                    </div>
                    
                    <div class="grid gap-[1rem]" aria-label="create role permission matrix">
                      <For each={PERMISSION_CATEGORIES}>
                        {(category) => (
                           <div class="grid gap-[0.5rem]">
                              <h5 class="m-0 text-[0.8rem] uppercase tracking-widest text-ink-2 font-semibold mb-[0.2rem]">{category.title}</h5>
                              <div class="grid gap-[0.5rem]">
                                <For each={permissionsByCategory(category.key)}>
                                  {(entry) => (
                                    <label class={`${permissionToggleClass} cursor-pointer`}>
                                      <div class="flex flex-col gap-[0.1rem] pr-[1rem]">
                                        <span class="text-[0.92rem] font-medium text-ink-0">{entry.label}</span>
                                        <span class="text-[0.8rem] text-ink-2 leading-tight">{entry.summary}</span>
                                      </div>
                                      <div class="relative inline-flex items-center cursor-pointer flex-shrink-0">
                                        <input
                                          type="checkbox"
                                          class="sr-only peer"
                                          checked={createPermissions().includes(entry.permission)}
                                          onChange={(event) =>
                                            setCreatePermissions((current) =>
                                              togglePermission(
                                                current,
                                                entry.permission,
                                                event.currentTarget.checked,
                                              ))}
                                        />
                                        <div class="w-10 h-6 bg-line-soft peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-brand rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-brand"></div>
                                      </div>
                                    </label>
                                  )}
                                </For>
                              </div>
                           </div>
                        )}
                      </For>
                    </div>
                  </section>

                  <div class="p-[1.5rem] border-t border-line-soft flex justify-end gap-[1rem] mt-[2rem] -mx-[2rem] px-[2rem]">
                    <button
                      class={actionButtonClass}
                      type="button"
                      onClick={() => setIsCreatingRole(false)}
                    >
                      Cancel
                    </button>
                    <button
                      class={primaryButtonClass}
                      type="submit"
                      disabled={props.isMutatingRoles || !isCreateRoleNameValid()}
                    >
                      {props.isMutatingRoles ? "Creating..." : "Create Role"}
                    </button>
                  </div>
                </form>
              </div>
            </Show>

            <Show when={!isCreatingRole() && selectedRole()}>
              {(roleAccessor) => (
                <div class="h-full flex flex-col relative pb-[6rem]">
                  {/* Sticky Header with Tabs */}
                  <header class="sticky top-0 z-10 bg-bg-1 px-[2rem] pt-[2rem] border-b border-line-soft">
                    <div class="flex items-center justify-between mb-[1.5rem]">
                      <div>
                        <h2 class="m-0 text-[1.4rem] font-semibold text-ink-0 flex items-center gap-[0.5rem]">
                          {roleAccessor().name}
                          <Show when={roleAccessor().isSystem}>
                             <span class={statusChipClass}>System</span>
                          </Show>
                        </h2>
                      </div>
                      <Show when={hasRoleDraftChanges() && !roleAccessor().isSystem}>
                        <div class="flex items-center gap-[0.5rem] bg-bg-2 px-[0.8rem] py-[0.4rem] rounded-[0.4rem] border border-line-soft">
                           <span class="text-[0.8rem] text-ink-1 font-medium mr-[0.5rem]">Unsaved changes</span>
                           <button type="button" class="text-[0.8rem] hover:underline text-ink-2" onClick={onResetRoleDraft}>Reset</button>
                           <button type="button" class="text-[0.8rem] bg-brand text-brand-ink px-[0.6rem] py-[0.2rem] rounded hover:brightness-110 font-medium" onClick={(e) => void onSaveRole(e as any)}>Save</button>
                        </div>
                      </Show>
                    </div>

                    <nav class="flex gap-[1.5rem]" aria-label="Role settings tabs">
                      <button
                        type="button"
                        class={`${tabButtonClass} ${activeTab() === "display" ? tabButtonActive : tabButtonInactive}`}
                        onClick={() => setActiveTab("display")}
                      >
                        Display
                      </button>
                      <button
                        type="button"
                        class={`${tabButtonClass} ${activeTab() === "permissions" ? tabButtonActive : tabButtonInactive}`}
                        onClick={() => setActiveTab("permissions")}
                      >
                        Permissions
                      </button>
                      <button
                        type="button"
                        class={`${tabButtonClass} ${activeTab() === "members" ? tabButtonActive : tabButtonInactive}`}
                        onClick={() => setActiveTab("members")}
                      >
                        Manage Members
                      </button>
                    </nav>
                  </header>

                  {/* Tab Content */}
                  <div class="flex-1 overflow-y-auto p-[2rem] max-w-[800px] w-full mx-auto">
                    <Switch>
                      <Match when={activeTab() === "display"}>
                        <form class={formClass} onSubmit={onSaveRole}>
                          <section class="grid gap-[1.5rem] max-w-[500px]">
                            <label class={fieldLabelClass}>
                              Role name
                              <input
                                class={fieldControlClass}
                                value={editName()}
                                onInput={(event) => setEditName(event.currentTarget.value)}
                                maxlength="32"
                                disabled={roleAccessor().isSystem || !props.canManageWorkspaceRoles}
                              />
                            </label>
                            <div class="flex items-center gap-[0.5rem]">
                              <button
                                type="button"
                                class={primaryButtonClass}
                                onClick={() => void onSaveRoleNameOnly()}
                                disabled={
                                  props.isMutatingRoles ||
                                  !isRoleNameValid() ||
                                  roleAccessor().isSystem ||
                                  !props.canManageWorkspaceRoles
                                }
                              >
                                Save name
                              </button>
                            </div>

                            <Show when={roleAccessor().isSystem}>
                              <div class="bg-bg-2 p-[1rem] rounded-[0.4rem] border border-line-soft">
                                <p class={mutedTextClass}>
                                  This is a system role. Its name and core permissions cannot be modified.
                                </p>
                                <p class="m-0 mt-[0.45rem] text-[0.8rem] text-ink-2">
                                  Select a custom role from the list to rename it.
                                </p>
                              </div>
                            </Show>
                          </section>

                          <Show when={props.canManageWorkspaceRoles && !roleAccessor().isSystem}>
                            <div class="mt-[4rem] border-t border-line-soft pt-[2rem]">
                              <h4 class="m-0 text-[1rem] font-medium text-danger mb-[0.5rem]">Danger Zone</h4>
                              <div class="flex items-center justify-between p-[1rem] bg-bg-2 border border-danger-panel-strong rounded-[0.4rem]">
                                <div>
                                  <h5 class="m-0 text-[0.92rem] text-ink-0 font-medium mb-[0.2rem]">Delete Role</h5>
                                  <p class="m-0 text-[0.8rem] text-ink-2">Once you delete a role, there is no going back.</p>
                                </div>
                                <button
                                  class="px-[1rem] py-[0.5rem] bg-danger text-danger-ink rounded-[0.4rem] font-medium text-[0.88rem] hover:brightness-110 transition-all"
                                  type="button"
                                  disabled={props.isMutatingRoles}
                                  onClick={() => void onDeleteRole()}
                                >
                                  Delete Role
                                </button>
                              </div>
                            </div>
                          </Show>
                        </form>
                      </Match>

                      <Match when={activeTab() === "permissions"}>
                        <form onSubmit={onSaveRole}>
                          <div class="mb-[1.5rem] flex items-center justify-between">
                            <p class={mutedTextClass}>
                              Control what members with this role can do in the workspace.
                            </p>
                            <span class="text-[0.8rem] bg-bg-2 px-[0.6rem] py-[0.2rem] rounded-[0.4rem] text-ink-2 font-medium">
                              {editPermissions().length} allowed
                            </span>
                          </div>

                          <div class="grid gap-[2rem]" aria-label="edit role permission matrix">
                            <For each={PERMISSION_CATEGORIES}>
                              {(category) => (
                                <section class="grid gap-[0.5rem]">
                                  <h6 class="m-0 text-[0.8rem] uppercase tracking-widest text-ink-2 font-semibold border-b border-line-soft pb-[0.4rem] mb-[0.4rem]">
                                    {category.title}
                                  </h6>
                                  <div class="grid gap-[0.5rem]">
                                    <For each={permissionsByCategory(category.key)}>
                                      {(entry) => (
                                        <label class={`${permissionToggleClass} ${roleAccessor().isSystem || !props.canManageWorkspaceRoles ? 'opacity-70 cursor-not-allowed' : 'cursor-pointer'}`}>
                                          <div class="flex flex-col gap-[0.1rem] pr-[1rem]">
                                            <span class="text-[0.92rem] font-medium text-ink-0">{entry.label}</span>
                                            <span class="text-[0.8rem] text-ink-2 leading-tight">{entry.summary}</span>
                                          </div>
                                          <div class="relative inline-flex items-center flex-shrink-0">
                                            <input
                                              type="checkbox"
                                              class="sr-only peer"
                                              checked={editPermissions().includes(entry.permission)}
                                              disabled={roleAccessor().isSystem || !props.canManageWorkspaceRoles}
                                              onChange={(event) =>
                                                setEditPermissions((current) =>
                                                  togglePermission(
                                                    current,
                                                    entry.permission,
                                                    event.currentTarget.checked,
                                                  ))}
                                            />
                                            <div class="w-10 h-6 bg-line-soft peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-brand rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-brand peer-disabled:opacity-50"></div>
                                          </div>
                                        </label>
                                      )}
                                    </For>
                                  </div>
                                </section>
                              )}
                            </For>
                          </div>
                        </form>
                      </Match>

                      <Match when={activeTab() === "members"}>
                        <div class="grid gap-[1.5rem]">
                           <div class="bg-bg-2 p-[1.5rem] border border-line-soft rounded-[0.6rem] max-w-[500px]">
                             <p class="m-0 text-[0.9rem] text-ink-1 mb-[1.5rem]">
                               Use the member role assignment tool below to grant or revoke this role for a specific user.
                             </p>
                             <div class="flex flex-col md:flex-row gap-[1rem] items-end">
                                <label class={`${fieldLabelClass} flex-1 w-full`}>
                                  Target user ULID
                                  <input
                                    class={fieldControlClass}
                                    value={props.targetUserIdInput}
                                    onInput={(event) => props.onTargetUserIdInput(event.currentTarget.value)}
                                    maxlength="26"
                                    placeholder="01ARZ..."
                                    disabled={!props.canManageMemberRoles}
                                  />
                                </label>
                                <div class="flex gap-[0.5rem] w-full md:w-auto">
                                  <button
                                    class={`${actionButtonClass} flex-1 md:flex-none border-brand text-brand hover:bg-brand hover:text-brand-ink disabled:hover:bg-transparent disabled:hover:text-brand`}
                                    type="button"
                                    disabled={props.isMutatingRoles || !props.canManageMemberRoles || !props.targetUserIdInput}
                                    onClick={() => {
                                      setAssignmentRoleId(roleAccessor().roleId);
                                      void onAssignRole();
                                    }}
                                  >
                                    Assign Role
                                  </button>
                                  <button
                                    class={`${actionButtonClass} flex-1 md:flex-none`}
                                    type="button"
                                    disabled={props.isMutatingRoles || !props.canManageMemberRoles || !props.targetUserIdInput}
                                    onClick={() => {
                                      setAssignmentRoleId(roleAccessor().roleId);
                                      void onUnassignRole();
                                    }}
                                  >
                                    Revoke
                                  </button>
                                </div>
                             </div>
                             <Show when={!props.canManageMemberRoles}>
                               <p class="mt-[0.5rem] text-[0.8rem] text-danger">You do not have permission to manage member roles.</p>
                             </Show>
                           </div>
                           
                           {/* Notice about member list integration */}
                           <div class="bg-bg-0 p-[1.5rem] rounded-[0.6rem] border border-line-soft border-dashed text-center">
                              <p class={mutedTextClass}>
                                For a full list of members and bulk assignments, visit the <strong class="text-ink-1">Members</strong> section in Workspace Settings.
                              </p>
                           </div>
                        </div>
                      </Match>
                    </Switch>
                  </div>

                  {/* Absolute positioning for the Save button so it doesn't get lost on long pages */}
                  {/* Sticky positioning for the Save button so it doesn't get lost on long pages */}
                  <Show when={activeTab() !== "members" && hasRoleDraftChanges() && !roleAccessor().isSystem}>
                    <div class="sticky bottom-0 left-0 right-0 bg-bg-1 p-[1.5rem] border-t border-line-soft flex items-center justify-between shadow-[0_-4px_12px_rgba(0,0,0,0.1)] z-10 mt-auto">
                       <span class="text-[0.9rem] text-ink-2">You have unsaved changes to this role.</span>
                       <div class="flex gap-[1rem]">
                         <button
                           class={actionButtonClass}
                           type="button"
                           onClick={onResetRoleDraft}
                           disabled={props.isMutatingRoles}
                         >
                           Reset
                         </button>
                         <button
                           class={primaryButtonClass}
                           type="button"
                           onClick={(e) => void onSaveRole(e as any)}
                           disabled={props.isMutatingRoles || !isRoleNameValid()}
                         >
                           {props.isMutatingRoles ? "Saving..." : "Save Changes"}
                         </button>
                       </div>
                    </div>
                  </Show>
                </div>
              )}
            </Show>
          </div>
        </div>
      </Show>

      {/* Global Status indicators */}
      <div class="fixed bottom-[2rem] left-1/2 -translate-x-1/2 z-50 flex flex-col gap-[0.5rem]">
        <Show when={props.roleManagementStatus}>
          <div class="px-[1.5rem] py-[0.8rem] rounded-[0.6rem] bg-bg-2 border border-ok text-[0.92rem] text-ok font-medium shadow-lg">
            {props.roleManagementStatus}
          </div>
        </Show>
        <Show when={props.roleManagementError || clientError()}>
          <div class="px-[1.5rem] py-[0.8rem] rounded-[0.6rem] bg-bg-2 border border-danger text-[0.92rem] text-danger font-medium shadow-lg">
            {props.roleManagementError || clientError()}
          </div>
        </Show>
      </div>

      <Show when={dangerModal()}>
        {(dangerModalAccessor) => (
          <div
            class="fixed inset-0 z-[80] grid place-items-center bg-bg-0/80 p-[1rem] backdrop-blur-sm"
            role="dialog"
            aria-modal="true"
            aria-label="Dangerous operation confirmation"
          >
            <section class="grid w-full max-w-[32rem] gap-[1rem] rounded-[0.6rem] border border-danger-panel-strong bg-bg-1 p-[1.5rem] text-ink-0 shadow-2xl">
              <h5 class="m-0 text-[1.2rem] font-semibold">{dangerModalAccessor().title}</h5>
              <p class="m-0 text-[0.95rem] text-ink-2 leading-relaxed">{dangerModalAccessor().message}</p>
              <div class="flex justify-end gap-[0.8rem] mt-[1rem]">
                <button
                  class={actionButtonClass}
                  type="button"
                  onClick={() => setDangerModal(null)}
                >
                  Cancel
                </button>
                <button
                  class="min-h-[2.2rem] rounded-[0.4rem] border border-danger bg-danger px-[1rem] py-[0.5rem] text-[0.92rem] font-medium text-danger-ink transition-colors duration-[120ms] ease-out hover:brightness-110"
                  type="button"
                  onClick={() => void onConfirmDangerModal()}
                >
                  {dangerModalAccessor().confirmLabel}
                </button>
              </div>
            </section>
          </div>
        )}
      </Show>
    </section>
  );
}
