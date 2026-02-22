import {
  createEffect,
  createSignal,
  untrack,
  type Accessor,
  type Setter,
} from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  userIdFromInput,
  workspaceRoleNameFromInput,
  type ChannelId,
  type ChannelPermissionSnapshot,
  type GuildId,
  type GuildRoleRecord,
  type PermissionName,
  type UserId,
  type WorkspaceRoleId,
} from "../../../domain/chat";
import {
  ApiError,
  assignGuildRole,
  createGuildRole,
  deleteGuildRole,
  fetchChannelPermissionSnapshot,
  fetchGuildRoles,
  reorderGuildRoles,
  unassignGuildRole,
  updateGuildRole,
} from "../../../lib/api";
import { mapError } from "../helpers";
import { sortWorkspaceRolesByPosition } from "../state/workspace-state";

export interface RoleManagementControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  setChannelPermissions: Setter<ChannelPermissionSnapshot | null>;
  roles?: Accessor<GuildRoleRecord[]>;
  setRoles?: Setter<GuildRoleRecord[]>;
  setWorkspaceRolesForGuild?: (
    guildId: GuildId,
    roles: ReadonlyArray<GuildRoleRecord>,
  ) => void;
  assignWorkspaceRoleToUser?: (
    guildId: GuildId,
    userId: UserId,
    roleId: WorkspaceRoleId,
  ) => void;
  unassignWorkspaceRoleFromUser?: (
    guildId: GuildId,
    userId: UserId,
    roleId: WorkspaceRoleId,
  ) => void;
}

export interface RoleManagementControllerDependencies {
  fetchGuildRoles: typeof fetchGuildRoles;
  createGuildRole: typeof createGuildRole;
  updateGuildRole: typeof updateGuildRole;
  deleteGuildRole: typeof deleteGuildRole;
  reorderGuildRoles: typeof reorderGuildRoles;
  assignGuildRole: typeof assignGuildRole;
  unassignGuildRole: typeof unassignGuildRole;
  fetchChannelPermissionSnapshot: typeof fetchChannelPermissionSnapshot;
  mapError: (error: unknown, fallback: string) => string;
}

export interface RoleManagementController {
  roles: Accessor<GuildRoleRecord[]>;
  isLoadingRoles: Accessor<boolean>;
  isMutatingRoles: Accessor<boolean>;
  roleManagementStatus: Accessor<string>;
  roleManagementError: Accessor<string>;
  refreshRoles: () => Promise<void>;
  createRole: (input: {
    name: string;
    permissions: PermissionName[];
    position?: number;
  }) => Promise<void>;
  updateRole: (
    roleId: WorkspaceRoleId,
    input: { name?: string; permissions?: PermissionName[] },
  ) => Promise<void>;
  deleteRole: (roleId: WorkspaceRoleId) => Promise<void>;
  reorderRoles: (roleIds: WorkspaceRoleId[]) => Promise<void>;
  assignRoleToMember: (targetUserIdInput: string, roleId: WorkspaceRoleId) => Promise<void>;
  unassignRoleFromMember: (
    targetUserIdInput: string,
    roleId: WorkspaceRoleId,
  ) => Promise<void>;
}

const DEFAULT_ROLE_MANAGEMENT_CONTROLLER_DEPENDENCIES: RoleManagementControllerDependencies = {
  fetchGuildRoles,
  createGuildRole,
  updateGuildRole,
  deleteGuildRole,
  reorderGuildRoles,
  assignGuildRole,
  unassignGuildRole,
  fetchChannelPermissionSnapshot,
  mapError,
};

function operationLabel(code: string): string {
  if (code === "forbidden") {
    return "Operation denied by workspace role policy.";
  }
  if (code === "quota_exceeded") {
    return "Workspace role limit reached.";
  }
  if (code === "not_found") {
    return "Target role or member was not found in this workspace.";
  }
  if (code === "invalid_request") {
    return "Role request was rejected by policy validation.";
  }
  return "Role operation failed.";
}

export function createRoleManagementController(
  options: RoleManagementControllerOptions,
  dependencies: Partial<RoleManagementControllerDependencies> = {},
): RoleManagementController {
  const deps = {
    ...DEFAULT_ROLE_MANAGEMENT_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const [localRoles, setLocalRoles] = createSignal<GuildRoleRecord[]>([]);
  const roles = options.roles ?? localRoles;
  const setRoles = options.setRoles ?? setLocalRoles;
  const [isLoadingRoles, setLoadingRoles] = createSignal(false);
  const [isMutatingRoles, setMutatingRoles] = createSignal(false);
  const [roleManagementStatus, setRoleManagementStatus] = createSignal("");
  const [roleManagementError, setRoleManagementError] = createSignal("");

  let loadVersion = 0;

  const refreshChannelPermissions = async (
    session: AuthSession,
    guildId: GuildId,
  ): Promise<void> => {
    const channelId = options.activeChannelId();
    if (!channelId || options.activeGuildId() !== guildId) {
      return;
    }
    try {
      const snapshot = await deps.fetchChannelPermissionSnapshot(session, guildId, channelId);
      options.setChannelPermissions(snapshot);
    } catch (error) {
      if (
        error instanceof ApiError &&
        (error.code === "forbidden" || error.code === "not_found")
      ) {
        options.setChannelPermissions(null);
      }
    }
  };

  const refreshRoles = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      setRoles([]);
      setLoadingRoles(false);
      return;
    }

    const requestVersion = ++loadVersion;
    setLoadingRoles(true);
    setRoleManagementError("");

    try {
      const response = await deps.fetchGuildRoles(session, guildId);
      if (requestVersion !== loadVersion) {
        return;
      }
      const orderedRoles = sortWorkspaceRolesByPosition(response.roles);
      setRoles(orderedRoles);
      options.setWorkspaceRolesForGuild?.(guildId, orderedRoles);
    } catch (error) {
      if (requestVersion !== loadVersion) {
        return;
      }
      setRoles([]);
      options.setWorkspaceRolesForGuild?.(guildId, []);
      setRoleManagementError(
        deps.mapError(error, "Unable to load workspace roles."),
      );
    } finally {
      if (requestVersion === loadVersion) {
        setLoadingRoles(false);
      }
    }
  };

  const runMutation = async (
    operation: () => Promise<void>,
    successMessage: string,
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId || isMutatingRoles()) {
      return;
    }

    setMutatingRoles(true);
    setRoleManagementError("");
    setRoleManagementStatus("");

    try {
      await operation();
      await refreshRoles();
      await refreshChannelPermissions(session, guildId);
      setRoleManagementStatus(successMessage);
    } catch (error) {
      if (error instanceof ApiError) {
        setRoleManagementError(operationLabel(error.code));
      } else {
        setRoleManagementError(deps.mapError(error, "Role operation failed."));
      }
    } finally {
      setMutatingRoles(false);
    }
  };

  const createRole = async (input: {
    name: string;
    permissions: PermissionName[];
    position?: number;
  }): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }
    let roleName;
    try {
      roleName = workspaceRoleNameFromInput(input.name);
    } catch {
      setRoleManagementError("Role name is invalid.");
      return;
    }
    await runMutation(async () => {
      await deps.createGuildRole(session, guildId, {
        name: roleName,
        permissions: input.permissions,
        position: input.position,
      });
    }, `Role ${roleName} created.`);
  };

  const updateRole = async (
    roleId: WorkspaceRoleId,
    input: { name?: string; permissions?: PermissionName[] },
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }
    let roleName: ReturnType<typeof workspaceRoleNameFromInput> | undefined;
    if (typeof input.name === "string") {
      try {
        roleName = workspaceRoleNameFromInput(input.name);
      } catch {
        setRoleManagementError("Role name is invalid.");
        return;
      }
    }
    await runMutation(async () => {
      await deps.updateGuildRole(session, guildId, roleId, {
        name: roleName,
        permissions: input.permissions,
      });
    }, "Role updated.");
  };

  const deleteRoleById = async (roleId: WorkspaceRoleId): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }

    await runMutation(async () => {
      await deps.deleteGuildRole(session, guildId, roleId);
    }, "Role deleted.");
  };

  const reorderRolesById = async (roleIds: WorkspaceRoleId[]): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }

    await runMutation(async () => {
      await deps.reorderGuildRoles(session, guildId, roleIds);
    }, "Role hierarchy reordered.");
  };

  const parseTargetUserId = (targetUserIdInput: string): UserId =>
    userIdFromInput(targetUserIdInput.trim());

  const assignRoleToMember = async (
    targetUserIdInput: string,
    roleId: WorkspaceRoleId,
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }
    let targetUserId: UserId;
    try {
      targetUserId = parseTargetUserId(targetUserIdInput);
    } catch {
      setRoleManagementError("Target user ID is invalid.");
      return;
    }

    await runMutation(async () => {
      await deps.assignGuildRole(session, guildId, roleId, targetUserId);
      options.assignWorkspaceRoleToUser?.(guildId, targetUserId, roleId);
    }, "Role assigned to member.");
  };

  const unassignRoleFromMember = async (
    targetUserIdInput: string,
    roleId: WorkspaceRoleId,
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }
    let targetUserId: UserId;
    try {
      targetUserId = parseTargetUserId(targetUserIdInput);
    } catch {
      setRoleManagementError("Target user ID is invalid.");
      return;
    }

    await runMutation(async () => {
      await deps.unassignGuildRole(session, guildId, roleId, targetUserId);
      options.unassignWorkspaceRoleFromUser?.(guildId, targetUserId, roleId);
    }, "Role removed from member.");
  };

  createEffect(() => {
    const session = options.session();
    const guildId = options.activeGuildId();
    loadVersion += 1;
    setRoleManagementStatus("");
    setRoleManagementError("");
    if (!session || !guildId) {
      setRoles([]);
      setLoadingRoles(false);
      return;
    }
    void untrack(() => refreshRoles());
  });

  return {
    roles,
    isLoadingRoles,
    isMutatingRoles,
    roleManagementStatus,
    roleManagementError,
    refreshRoles,
    createRole,
    updateRole,
    deleteRole: deleteRoleById,
    reorderRoles: reorderRolesById,
    assignRoleToMember,
    unassignRoleFromMember,
  };
}
