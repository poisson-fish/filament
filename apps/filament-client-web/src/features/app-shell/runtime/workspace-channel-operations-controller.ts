import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  channelKindFromInput,
  channelNameFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  type ChannelId,
  type ChannelKindName,
  type GuildId,
  type GuildVisibility,
  type WorkspaceRecord,
} from "../../../domain/chat";
import {
  createChannel,
  createGuild,
} from "../../../lib/api";
import { mapError, upsertWorkspace } from "../helpers";
import type { OverlayPanel } from "../types";

export interface WorkspaceChannelOperationsControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  createGuildName: Accessor<string>;
  createGuildVisibility: Accessor<GuildVisibility>;
  createChannelName: Accessor<string>;
  createChannelKind: Accessor<ChannelKindName>;
  isCreatingWorkspace: Accessor<boolean>;
  isCreatingChannel: Accessor<boolean>;
  newChannelName: Accessor<string>;
  newChannelKind: Accessor<ChannelKindName>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setActiveGuildId: Setter<GuildId | null>;
  setActiveChannelId: Setter<ChannelId | null>;
  setCreateChannelKind: Setter<ChannelKindName>;
  setWorkspaceError: Setter<string>;
  setCreatingWorkspace: Setter<boolean>;
  setMessageStatus: Setter<string>;
  setActiveOverlayPanel: Setter<OverlayPanel | null>;
  setChannelCreateError: Setter<string>;
  setCreatingChannel: Setter<boolean>;
  setNewChannelName: Setter<string>;
  setNewChannelKind: Setter<ChannelKindName>;
}

export interface WorkspaceChannelOperationsControllerDependencies {
  createGuild: typeof createGuild;
  createChannel: typeof createChannel;
  mapError: (error: unknown, fallback: string) => string;
  upsertWorkspace: (
    existing: WorkspaceRecord[],
    guildId: GuildId,
    updater: (workspace: WorkspaceRecord) => WorkspaceRecord,
  ) => WorkspaceRecord[];
}

export interface WorkspaceChannelOperationsController {
  createWorkspace: (event: SubmitEvent) => Promise<void>;
  createNewChannel: (event: SubmitEvent) => Promise<void>;
}

const DEFAULT_WORKSPACE_CHANNEL_OPERATIONS_CONTROLLER_DEPENDENCIES: WorkspaceChannelOperationsControllerDependencies =
  {
    createGuild,
    createChannel,
    mapError,
    upsertWorkspace,
  };

export function createWorkspaceChannelOperationsController(
  options: WorkspaceChannelOperationsControllerOptions,
  dependencies: Partial<WorkspaceChannelOperationsControllerDependencies> = {},
): WorkspaceChannelOperationsController {
  const deps = {
    ...DEFAULT_WORKSPACE_CHANNEL_OPERATIONS_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const createWorkspace = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    const session = options.session();
    if (!session) {
      options.setWorkspaceError("Missing auth session.");
      return;
    }
    if (options.isCreatingWorkspace()) {
      return;
    }

    options.setWorkspaceError("");
    options.setCreatingWorkspace(true);
    try {
      const guild = await deps.createGuild(session, {
        name: guildNameFromInput(options.createGuildName()),
        visibility: guildVisibilityFromInput(options.createGuildVisibility()),
      });
      const channel = await deps.createChannel(session, guild.guildId, {
        name: channelNameFromInput(options.createChannelName()),
        kind: channelKindFromInput(options.createChannelKind()),
      });
      const createdWorkspace: WorkspaceRecord = {
        guildId: guild.guildId,
        guildName: guild.name,
        visibility: guild.visibility,
        channels: [channel],
      };
      options.setWorkspaces((existing) => [...existing, createdWorkspace]);
      options.setActiveGuildId(createdWorkspace.guildId);
      options.setActiveChannelId(channel.channelId);
      options.setCreateChannelKind("text");
      options.setMessageStatus("Workspace created.");
      options.setActiveOverlayPanel(null);
    } catch (error) {
      options.setWorkspaceError(deps.mapError(error, "Unable to create workspace."));
    } finally {
      options.setCreatingWorkspace(false);
    }
  };

  const createNewChannel = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      options.setChannelCreateError("Select a workspace first.");
      return;
    }
    if (options.isCreatingChannel()) {
      return;
    }

    options.setChannelCreateError("");
    options.setCreatingChannel(true);
    try {
      const created = await deps.createChannel(session, guildId, {
        name: channelNameFromInput(options.newChannelName()),
        kind: channelKindFromInput(options.newChannelKind()),
      });
      options.setWorkspaces((existing) =>
        deps.upsertWorkspace(existing, guildId, (workspace) => {
          if (
            workspace.channels.some(
              (channel) => channel.channelId === created.channelId,
            )
          ) {
            return workspace;
          }
          return {
            ...workspace,
            channels: [...workspace.channels, created],
          };
        }),
      );
      options.setActiveChannelId(created.channelId);
      options.setActiveOverlayPanel(null);
      options.setNewChannelName("backend");
      options.setNewChannelKind("text");
      options.setMessageStatus("Channel created.");
    } catch (error) {
      options.setChannelCreateError(deps.mapError(error, "Unable to create channel."));
    } finally {
      options.setCreatingChannel(false);
    }
  };

  return {
    createWorkspace,
    createNewChannel,
  };
}
