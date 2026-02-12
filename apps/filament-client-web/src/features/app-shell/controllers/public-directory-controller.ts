import { createEffect, untrack, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  directoryJoinErrorCodeFromInput,
  type ChannelId,
  type DirectoryJoinErrorCode,
  type DirectoryJoinResult,
  type GuildId,
  type GuildRecord,
  type WorkspaceRecord,
} from "../../../domain/chat";
import {
  ApiError,
  fetchGuildChannels,
  fetchGuilds,
  fetchPublicGuildDirectory,
  joinPublicGuild,
} from "../../../lib/api";
import { mapError } from "../helpers";
import type { PublicDirectoryJoinStatus } from "../types";
import {
  filterAccessibleWorkspaces,
  resolveWorkspaceSelection,
  type WorkspaceSelection,
} from "./workspace-controller";

export interface PublicDirectoryControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  publicGuildSearchQuery: Accessor<string>;
  isSearchingPublicGuilds: Accessor<boolean>;
  publicGuildJoinStatusByGuildId: Accessor<Record<string, PublicDirectoryJoinStatus>>;
  setSearchingPublicGuilds: Setter<boolean>;
  setPublicGuildSearchError: Setter<string>;
  setPublicGuildDirectory: Setter<GuildRecord[]>;
  setPublicGuildJoinStatusByGuildId: Setter<Record<string, PublicDirectoryJoinStatus>>;
  setPublicGuildJoinErrorByGuildId: Setter<Record<string, string>>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setActiveGuildId: Setter<GuildId | null>;
  setActiveChannelId: Setter<ChannelId | null>;
}

export interface PublicDirectoryControllerDependencies {
  fetchPublicGuildDirectory: typeof fetchPublicGuildDirectory;
  joinPublicGuild: typeof joinPublicGuild;
  fetchGuilds: typeof fetchGuilds;
  fetchGuildChannels: typeof fetchGuildChannels;
  mapError: (error: unknown, fallback: string) => string;
  filterAccessibleWorkspaces: (
    workspaces: Array<WorkspaceRecord | null>,
  ) => WorkspaceRecord[];
  resolveWorkspaceSelection: (
    workspaces: WorkspaceRecord[],
    selectedGuildId: GuildId | null,
    selectedChannelId: ChannelId | null,
  ) => WorkspaceSelection;
}

export interface PublicDirectoryController {
  loadPublicGuildDirectory: (query?: string) => Promise<void>;
  runPublicGuildSearch: (event: SubmitEvent) => Promise<void>;
  joinGuildFromDirectory: (guildId: GuildId) => Promise<void>;
}

const DEFAULT_PUBLIC_DIRECTORY_CONTROLLER_DEPENDENCIES: PublicDirectoryControllerDependencies = {
  fetchPublicGuildDirectory,
  joinPublicGuild,
  fetchGuilds,
  fetchGuildChannels,
  mapError,
  filterAccessibleWorkspaces,
  resolveWorkspaceSelection,
};

function directoryJoinStatusFromResult(result: DirectoryJoinResult): PublicDirectoryJoinStatus {
  if (result.outcome === "accepted" || result.outcome === "already_member") {
    return "joined";
  }
  if (result.outcome === "rejected_user_ban" || result.outcome === "rejected_ip_ban") {
    return "banned";
  }
  return "join_failed";
}

export function directoryJoinErrorMessageForCode(code: DirectoryJoinErrorCode): string {
  if (code === "directory_join_user_banned") {
    return "Join blocked by a workspace user-ban.";
  }
  if (code === "directory_join_ip_banned") {
    return "Join blocked by a workspace IP moderation policy.";
  }
  if (code === "rate_limited") {
    return "Too many join attempts. Please wait and retry.";
  }
  if (code === "forbidden") {
    return "Permission denied for this join request.";
  }
  if (code === "directory_join_not_allowed" || code === "not_found") {
    return "Workspace is not joinable from the public directory.";
  }
  return "Unable to join this workspace right now.";
}

function directoryJoinFailureFromError(
  error: unknown,
  mapErrorFn: (error: unknown, fallback: string) => string,
): { status: PublicDirectoryJoinStatus; message: string } {
  if (error instanceof ApiError) {
    const code = directoryJoinErrorCodeFromInput(error.code);
    if (code === "directory_join_user_banned" || code === "directory_join_ip_banned") {
      return {
        status: "banned",
        message: directoryJoinErrorMessageForCode(code),
      };
    }
    return {
      status: "join_failed",
      message: directoryJoinErrorMessageForCode(code),
    };
  }
  return {
    status: "join_failed",
    message: mapErrorFn(error, "Unable to join this workspace right now."),
  };
}

export function createPublicDirectoryController(
  options: PublicDirectoryControllerOptions,
  dependencies: Partial<PublicDirectoryControllerDependencies> = {},
): PublicDirectoryController {
  const deps = {
    ...DEFAULT_PUBLIC_DIRECTORY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };
  let directoryRequestVersion = 0;
  let joinRequestVersion = 0;
  const joinRequestByGuild = new Map<string, number>();

  const clearJoinState = (): void => {
    options.setPublicGuildJoinStatusByGuildId({});
    options.setPublicGuildJoinErrorByGuildId({});
    joinRequestByGuild.clear();
    joinRequestVersion += 1;
  };

  const refreshWorkspacesAfterJoin = async (session: AuthSession): Promise<void> => {
    const selectedGuildId = options.activeGuildId();
    const selectedChannelId = options.activeChannelId();
    const guilds = await deps.fetchGuilds(session);
    const workspacesWithChannels = await Promise.all(
      guilds.map(async (guild) => {
        try {
          return {
            guildId: guild.guildId,
            guildName: guild.name,
            visibility: guild.visibility,
            channels: await deps.fetchGuildChannels(session, guild.guildId),
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
    const filtered = deps.filterAccessibleWorkspaces(workspacesWithChannels);
    options.setWorkspaces(filtered);
    const nextSelection = deps.resolveWorkspaceSelection(
      filtered,
      selectedGuildId,
      selectedChannelId,
    );
    options.setActiveGuildId(nextSelection.guildId);
    options.setActiveChannelId(nextSelection.channelId);
  };

  const loadPublicGuildDirectory = async (query?: string): Promise<void> => {
    const session = options.session();
    if (!session) {
      options.setPublicGuildDirectory([]);
      clearJoinState();
      return;
    }
    if (options.isSearchingPublicGuilds()) {
      return;
    }
    const requestVersion = ++directoryRequestVersion;
    options.setSearchingPublicGuilds(true);
    options.setPublicGuildSearchError("");
    try {
      const directory = await deps.fetchPublicGuildDirectory(session, {
        query,
        limit: 20,
      });
      if (requestVersion !== directoryRequestVersion) {
        return;
      }
      options.setPublicGuildDirectory(directory.guilds);
    } catch (error) {
      if (requestVersion !== directoryRequestVersion) {
        return;
      }
      options.setPublicGuildSearchError(
        mapError(error, "Unable to load public workspace directory."),
      );
      options.setPublicGuildDirectory([]);
    } finally {
      if (requestVersion === directoryRequestVersion) {
        options.setSearchingPublicGuilds(false);
      }
    }
  };

  const runPublicGuildSearch = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    await loadPublicGuildDirectory(options.publicGuildSearchQuery());
  };

  const joinGuildFromDirectory = async (guildId: GuildId): Promise<void> => {
    const session = options.session();
    if (!session) {
      return;
    }
    const guildKey = guildId as string;
    if (options.publicGuildJoinStatusByGuildId()[guildKey] === "joining") {
      return;
    }

    const requestVersion = ++joinRequestVersion;
    joinRequestByGuild.set(guildKey, requestVersion);
    options.setPublicGuildJoinStatusByGuildId((existing) => ({
      ...existing,
      [guildKey]: "joining",
    }));
    options.setPublicGuildJoinErrorByGuildId((existing) => {
      if (typeof existing[guildKey] === "undefined") {
        return existing;
      }
      const next = { ...existing };
      delete next[guildKey];
      return next;
    });

    try {
      const result = await deps.joinPublicGuild(session, guildId);
      if (joinRequestByGuild.get(guildKey) !== requestVersion) {
        return;
      }
      const status = directoryJoinStatusFromResult(result);
      options.setPublicGuildJoinStatusByGuildId((existing) => ({
        ...existing,
        [guildKey]: status,
      }));
      if (status === "joined") {
        try {
          await refreshWorkspacesAfterJoin(session);
        } catch (refreshError) {
          if (joinRequestByGuild.get(guildKey) !== requestVersion) {
            return;
          }
          options.setPublicGuildJoinErrorByGuildId((existing) => ({
            ...existing,
            [guildKey]: deps.mapError(
              refreshError,
              "Workspace joined, but workspace list refresh failed.",
            ),
          }));
        }
      }
    } catch (error) {
      if (joinRequestByGuild.get(guildKey) !== requestVersion) {
        return;
      }
      const failure = directoryJoinFailureFromError(error, deps.mapError);
      options.setPublicGuildJoinStatusByGuildId((existing) => ({
        ...existing,
        [guildKey]: failure.status,
      }));
      options.setPublicGuildJoinErrorByGuildId((existing) => ({
        ...existing,
        [guildKey]: failure.message,
      }));
    }
  };

  createEffect(() => {
    const session = options.session();
    directoryRequestVersion += 1;
    if (!session) {
      options.setSearchingPublicGuilds(false);
      options.setPublicGuildDirectory([]);
      options.setPublicGuildSearchError("");
      clearJoinState();
      return;
    }
    void untrack(() => loadPublicGuildDirectory());
  });

  return {
    loadPublicGuildDirectory,
    runPublicGuildSearch,
    joinGuildFromDirectory,
  };
}
