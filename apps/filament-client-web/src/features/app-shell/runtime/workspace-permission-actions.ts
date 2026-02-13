import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  ChannelId,
  ChannelPermissionSnapshot,
  GuildId,
  WorkspaceRecord,
} from "../../../domain/chat";
import { fetchChannelPermissionSnapshot } from "../../../lib/api";
import {
  pruneWorkspaceChannel,
  shouldResetChannelPermissionsForError,
} from "../controllers/workspace-controller";

export interface WorkspacePermissionActionsOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  setChannelPermissions: Setter<ChannelPermissionSnapshot | null>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  refreshRoles: () => Promise<void> | void;
}

export interface WorkspacePermissionActionsDependencies {
  fetchChannelPermissionSnapshot: typeof fetchChannelPermissionSnapshot;
  pruneWorkspaceChannel: typeof pruneWorkspaceChannel;
  shouldResetChannelPermissionsForError: typeof shouldResetChannelPermissionsForError;
}

const DEFAULT_WORKSPACE_PERMISSION_ACTIONS_DEPENDENCIES: WorkspacePermissionActionsDependencies =
  {
    fetchChannelPermissionSnapshot,
    pruneWorkspaceChannel,
    shouldResetChannelPermissionsForError,
  };

export function createWorkspacePermissionActions(
  options: WorkspacePermissionActionsOptions,
  dependencies: Partial<WorkspacePermissionActionsDependencies> = {},
) {
  const deps = {
    ...DEFAULT_WORKSPACE_PERMISSION_ACTIONS_DEPENDENCIES,
    ...dependencies,
  };

  const refreshWorkspacePermissionStateFromGateway = async (
    guildId: GuildId,
  ): Promise<void> => {
    const session = options.session();
    const activeGuildId = options.activeGuildId();
    const activeChannelId = options.activeChannelId();
    if (!session || !activeGuildId || activeGuildId !== guildId) {
      return;
    }

    void options.refreshRoles();
    if (!activeChannelId) {
      return;
    }

    try {
      const snapshot = await deps.fetchChannelPermissionSnapshot(
        session,
        activeGuildId,
        activeChannelId,
      );
      options.setChannelPermissions(snapshot);
    } catch (error) {
      if (deps.shouldResetChannelPermissionsForError(error)) {
        options.setChannelPermissions(null);
        options.setWorkspaces((existing) =>
          deps.pruneWorkspaceChannel(existing, activeGuildId, activeChannelId),
        );
      }
    }
  };

  return {
    refreshWorkspacePermissionStateFromGateway,
  };
}
