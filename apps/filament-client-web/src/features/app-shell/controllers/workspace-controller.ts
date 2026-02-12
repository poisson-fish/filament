import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  ChannelId,
  ChannelPermissionSnapshot,
  GuildId,
  WorkspaceRecord,
} from "../../../domain/chat";
import {
  ApiError,
  fetchChannelPermissionSnapshot,
  fetchGuildChannels,
  fetchGuilds,
} from "../../../lib/api";

export interface WorkspaceSelection {
  guildId: GuildId | null;
  channelId: ChannelId | null;
}

export interface WorkspaceBootstrapControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setActiveGuildId: Setter<GuildId | null>;
  setActiveChannelId: Setter<ChannelId | null>;
  setWorkspaceBootstrapDone: Setter<boolean>;
}

export interface WorkspaceSelectionControllerOptions {
  workspaces: Accessor<WorkspaceRecord[]>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  setActiveGuildId: Setter<GuildId | null>;
  setActiveChannelId: Setter<ChannelId | null>;
}

export interface ChannelPermissionsControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setChannelPermissions: Setter<ChannelPermissionSnapshot | null>;
}

export function filterAccessibleWorkspaces(
  workspaces: Array<WorkspaceRecord | null>,
): WorkspaceRecord[] {
  return workspaces.filter(
    (workspace): workspace is WorkspaceRecord =>
      workspace !== null && workspace.channels.length > 0,
  );
}

export function resolveWorkspaceSelection(
  workspaces: WorkspaceRecord[],
  selectedGuildId: GuildId | null,
  selectedChannelId: ChannelId | null,
): WorkspaceSelection {
  const selectedWorkspace =
    (selectedGuildId &&
      workspaces.find((workspace) => workspace.guildId === selectedGuildId)) ??
    workspaces[0] ??
    null;

  if (!selectedWorkspace) {
    return {
      guildId: null,
      channelId: null,
    };
  }

  const selectedChannel =
    (selectedChannelId &&
      selectedWorkspace.channels.find((channel) => channel.channelId === selectedChannelId)) ??
    selectedWorkspace.channels[0] ??
    null;

  return {
    guildId: selectedWorkspace.guildId,
    channelId: selectedChannel?.channelId ?? null,
  };
}

export function pruneWorkspaceChannel(
  workspaces: WorkspaceRecord[],
  guildId: GuildId,
  channelId: ChannelId,
): WorkspaceRecord[] {
  const updated = workspaces.map((workspace) => {
    if (workspace.guildId !== guildId) {
      return workspace;
    }
    return {
      ...workspace,
      channels: workspace.channels.filter(
        (channel) => channel.channelId !== channelId,
      ),
    };
  });
  return filterAccessibleWorkspaces(updated);
}

export function createWorkspaceBootstrapController(
  options: WorkspaceBootstrapControllerOptions,
): void {
  createEffect(() => {
    const session = options.session();
    if (!session) {
      options.setWorkspaces([]);
      options.setWorkspaceBootstrapDone(true);
      return;
    }

    let cancelled = false;
    options.setWorkspaceBootstrapDone(false);

    const bootstrap = async () => {
      try {
        const guilds = await fetchGuilds(session);
        const workspacesWithChannels = await Promise.all(
          guilds.map(async (guild) => {
            try {
              return {
                guildId: guild.guildId,
                guildName: guild.name,
                visibility: guild.visibility,
                channels: await fetchGuildChannels(session, guild.guildId),
              };
            } catch (error) {
              if (
                error instanceof ApiError &&
                (error.code === "forbidden" || error.code === "not_found")
              ) {
                return null;
              }
              throw error;
            }
          }),
        );
        if (cancelled) {
          return;
        }
        const filtered = filterAccessibleWorkspaces(workspacesWithChannels);
        options.setWorkspaces(filtered);
        const nextSelection = resolveWorkspaceSelection(
          filtered,
          options.activeGuildId(),
          options.activeChannelId(),
        );
        options.setActiveGuildId(nextSelection.guildId);
        options.setActiveChannelId(nextSelection.channelId);
      } catch {
        if (!cancelled) {
          options.setWorkspaces([]);
          options.setActiveGuildId(null);
          options.setActiveChannelId(null);
        }
      } finally {
        if (!cancelled) {
          options.setWorkspaceBootstrapDone(true);
        }
      }
    };

    void bootstrap();

    onCleanup(() => {
      cancelled = true;
    });
  });
}

export function createWorkspaceSelectionController(
  options: WorkspaceSelectionControllerOptions,
): void {
  createEffect(() => {
    const nextSelection = resolveWorkspaceSelection(
      options.workspaces(),
      options.activeGuildId(),
      options.activeChannelId(),
    );
    if (nextSelection.guildId !== options.activeGuildId()) {
      options.setActiveGuildId(nextSelection.guildId);
    }
    if (nextSelection.channelId !== options.activeChannelId()) {
      options.setActiveChannelId(nextSelection.channelId);
    }
  });
}

export function createChannelPermissionsController(
  options: ChannelPermissionsControllerOptions,
): void {
  createEffect(() => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId) {
      options.setChannelPermissions(null);
      return;
    }

    let cancelled = false;
    const loadPermissions = async () => {
      try {
        const snapshot = await fetchChannelPermissionSnapshot(
          session,
          guildId,
          channelId,
        );
        if (!cancelled) {
          options.setChannelPermissions(snapshot);
        }
      } catch (error) {
        if (cancelled) {
          return;
        }
        options.setChannelPermissions(null);
        if (
          error instanceof ApiError &&
          (error.code === "forbidden" || error.code === "not_found")
        ) {
          options.setWorkspaces((existing) =>
            pruneWorkspaceChannel(existing, guildId, channelId),
          );
        }
      }
    };
    void loadPermissions();

    onCleanup(() => {
      cancelled = true;
    });
  });
}
