import { createSignal } from "solid-js";
import type {
  ChannelId,
  ChannelKindName,
  ChannelPermissionSnapshot,
  FriendRecord,
  FriendRequestList,
  GuildId,
  GuildRoleRecord,
  GuildRecord,
  GuildVisibility,
  PermissionName,
  RoleName,
  SearchResults,
  UserId,
  WorkspaceRoleId,
  WorkspaceRecord,
} from "../../../domain/chat";
import type { PublicDirectoryJoinStatus } from "../types";

const MAX_TRACKED_WORKSPACE_ROLES = 64;
const MAX_TRACKED_ROLE_ASSIGNMENTS_PER_USER = 64;
const MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES = 16;

export interface WorkspaceChannelOverrideRecord {
  targetKind: "legacy_role";
  role: RoleName;
  allow: PermissionName[];
  deny: PermissionName[];
  updatedAtUnix: number | null;
}

export type WorkspaceRolesByGuildId = Record<string, GuildRoleRecord[]>;
export type WorkspaceUserRolesByGuildId = Record<
  string,
  Record<string, WorkspaceRoleId[]>
>;
export type WorkspaceChannelOverridesByGuildId = Record<
  string,
  Record<string, WorkspaceChannelOverrideRecord[]>
>;

export function sortWorkspaceRolesByPosition(
  roles: ReadonlyArray<GuildRoleRecord>,
): GuildRoleRecord[] {
  const deduplicated = new Map<string, GuildRoleRecord>();
  for (const role of roles) {
    deduplicated.set(role.roleId, role);
    if (deduplicated.size >= MAX_TRACKED_WORKSPACE_ROLES) {
      break;
    }
  }
  return [...deduplicated.values()].sort((left, right) => {
    if (left.position !== right.position) {
      return right.position - left.position;
    }
    if (left.isSystem !== right.isSystem) {
      return left.isSystem ? 1 : -1;
    }
    return left.roleId.localeCompare(right.roleId);
  });
}

function createWorkspaceChannelState() {
  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
  const [activeGuildId, setActiveGuildId] = createSignal<GuildId | null>(null);
  const [activeChannelId, setActiveChannelId] = createSignal<ChannelId | null>(null);
  const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);

  const [createGuildName, setCreateGuildName] = createSignal("Security Ops");
  const [createGuildVisibility, setCreateGuildVisibility] = createSignal<GuildVisibility>("private");
  const [createChannelName, setCreateChannelName] = createSignal("incident-room");
  const [createChannelKind, setCreateChannelKind] = createSignal<ChannelKindName>("text");
  const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
  const [workspaceError, setWorkspaceError] = createSignal("");
  const [workspaceSettingsName, setWorkspaceSettingsName] = createSignal("");
  const [workspaceSettingsVisibility, setWorkspaceSettingsVisibility] =
    createSignal<GuildVisibility>("private");
  const [isSavingWorkspaceSettings, setSavingWorkspaceSettings] = createSignal(false);
  const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("");
  const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("");

  const [newChannelName, setNewChannelName] = createSignal("backend");
  const [newChannelKind, setNewChannelKind] = createSignal<ChannelKindName>("text");
  const [isCreatingChannel, setCreatingChannel] = createSignal(false);
  const [channelCreateError, setChannelCreateError] = createSignal("");

  const [channelPermissions, setChannelPermissions] =
    createSignal<ChannelPermissionSnapshot | null>(null);
  const [workspaceRolesByGuildId, setWorkspaceRolesByGuildId] =
    createSignal<WorkspaceRolesByGuildId>({});
  const [workspaceUserRolesByGuildId, setWorkspaceUserRolesByGuildId] =
    createSignal<WorkspaceUserRolesByGuildId>({});
  const [workspaceChannelOverridesByGuildId, setWorkspaceChannelOverridesByGuildId] =
    createSignal<WorkspaceChannelOverridesByGuildId>({});

  const setWorkspaceRolesForGuild = (
    guildId: GuildId,
    roles: ReadonlyArray<GuildRoleRecord>,
  ): void => {
    setWorkspaceRolesByGuildId((existing) => ({
      ...existing,
      [guildId]: sortWorkspaceRolesByPosition(roles),
    }));
  };

  const assignWorkspaceRoleToUser = (
    guildId: GuildId,
    userId: UserId,
    roleId: WorkspaceRoleId,
  ): void => {
    setWorkspaceUserRolesByGuildId((existing) => {
      const guildEntries = existing[guildId] ?? {};
      const current = guildEntries[userId] ?? [];
      if (current.includes(roleId)) {
        return existing;
      }
      if (current.length >= MAX_TRACKED_ROLE_ASSIGNMENTS_PER_USER) {
        return existing;
      }
      return {
        ...existing,
        [guildId]: {
          ...guildEntries,
          [userId]: [...current, roleId],
        },
      };
    });
  };

  const unassignWorkspaceRoleFromUser = (
    guildId: GuildId,
    userId: UserId,
    roleId: WorkspaceRoleId,
  ): void => {
    setWorkspaceUserRolesByGuildId((existing) => {
      const guildEntries = existing[guildId];
      if (!guildEntries) {
        return existing;
      }
      const current = guildEntries[userId];
      if (!current || current.length === 0) {
        return existing;
      }
      const nextRoles = current.filter((entry) => entry !== roleId);
      if (nextRoles.length === current.length) {
        return existing;
      }
      if (nextRoles.length > 0) {
        return {
          ...existing,
          [guildId]: {
            ...guildEntries,
            [userId]: nextRoles,
          },
        };
      }
      const nextGuildEntries = { ...guildEntries };
      delete nextGuildEntries[userId];
      return {
        ...existing,
        [guildId]: nextGuildEntries,
      };
    });
  };

  const setLegacyChannelOverride = (
    guildId: GuildId,
    channelId: ChannelId,
    role: RoleName,
    allow: ReadonlyArray<PermissionName>,
    deny: ReadonlyArray<PermissionName>,
    updatedAtUnix: number | null,
  ): void => {
    setWorkspaceChannelOverridesByGuildId((existing) => {
      const guildOverrides = existing[guildId] ?? {};
      const channelOverrides = guildOverrides[channelId] ?? [];
      const nextAllow = [...new Set(allow)];
      const nextDeny = [...new Set(deny)].filter(
        (permission) => !nextAllow.includes(permission),
      );
      const nextEntry: WorkspaceChannelOverrideRecord = {
        targetKind: "legacy_role",
        role,
        allow: nextAllow,
        deny: nextDeny,
        updatedAtUnix,
      };

      const targetIndex = channelOverrides.findIndex(
        (entry) =>
          entry.targetKind === "legacy_role" &&
          entry.role === role,
      );
      const nextChannelOverrides =
        targetIndex < 0
          ? [...channelOverrides, nextEntry]
          : channelOverrides.map((entry, index) =>
            index === targetIndex ? nextEntry : entry,
          );
      const boundedChannelOverrides =
        nextChannelOverrides.length > MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES
          ? nextChannelOverrides.slice(-MAX_TRACKED_LEGACY_CHANNEL_OVERRIDES)
          : nextChannelOverrides;

      return {
        ...existing,
        [guildId]: {
          ...guildOverrides,
          [channelId]: boundedChannelOverrides,
        },
      };
    });
  };

  return {
    workspaces,
    setWorkspaces,
    activeGuildId,
    setActiveGuildId,
    activeChannelId,
    setActiveChannelId,
    workspaceBootstrapDone,
    setWorkspaceBootstrapDone,
    createGuildName,
    setCreateGuildName,
    createGuildVisibility,
    setCreateGuildVisibility,
    createChannelName,
    setCreateChannelName,
    createChannelKind,
    setCreateChannelKind,
    isCreatingWorkspace,
    setCreatingWorkspace,
    workspaceError,
    setWorkspaceError,
    workspaceSettingsName,
    setWorkspaceSettingsName,
    workspaceSettingsVisibility,
    setWorkspaceSettingsVisibility,
    isSavingWorkspaceSettings,
    setSavingWorkspaceSettings,
    workspaceSettingsStatus,
    setWorkspaceSettingsStatus,
    workspaceSettingsError,
    setWorkspaceSettingsError,
    newChannelName,
    setNewChannelName,
    newChannelKind,
    setNewChannelKind,
    isCreatingChannel,
    setCreatingChannel,
    channelCreateError,
    setChannelCreateError,
    channelPermissions,
    setChannelPermissions,
    workspaceRolesByGuildId,
    setWorkspaceRolesByGuildId,
    setWorkspaceRolesForGuild,
    workspaceUserRolesByGuildId,
    setWorkspaceUserRolesByGuildId,
    assignWorkspaceRoleToUser,
    unassignWorkspaceRoleFromUser,
    workspaceChannelOverridesByGuildId,
    setWorkspaceChannelOverridesByGuildId,
    setLegacyChannelOverride,
  };
}

function createFriendshipsState() {
  const [friendRecipientUserIdInput, setFriendRecipientUserIdInput] = createSignal("");
  const [friends, setFriends] = createSignal<FriendRecord[]>([]);
  const [friendRequests, setFriendRequests] = createSignal<FriendRequestList>({
    incoming: [],
    outgoing: [],
  });
  const [isRunningFriendAction, setRunningFriendAction] = createSignal(false);
  const [friendStatus, setFriendStatus] = createSignal("");
  const [friendError, setFriendError] = createSignal("");

  return {
    friendRecipientUserIdInput,
    setFriendRecipientUserIdInput,
    friends,
    setFriends,
    friendRequests,
    setFriendRequests,
    isRunningFriendAction,
    setRunningFriendAction,
    friendStatus,
    setFriendStatus,
    friendError,
    setFriendError,
  };
}

function createDiscoveryState() {
  const [publicGuildSearchQuery, setPublicGuildSearchQuery] = createSignal("");
  const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
  const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
  const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<GuildRecord[]>([]);
  const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
    Record<string, PublicDirectoryJoinStatus>
  >({});
  const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
    Record<string, string>
  >({});

  const [searchQuery, setSearchQuery] = createSignal("");
  const [searchError, setSearchError] = createSignal("");
  const [isSearching, setSearching] = createSignal(false);
  const [searchResults, setSearchResults] = createSignal<SearchResults | null>(null);
  const [isRunningSearchOps, setRunningSearchOps] = createSignal(false);
  const [searchOpsStatus, setSearchOpsStatus] = createSignal("");

  return {
    publicGuildSearchQuery,
    setPublicGuildSearchQuery,
    isSearchingPublicGuilds,
    setSearchingPublicGuilds,
    publicGuildSearchError,
    setPublicGuildSearchError,
    publicGuildDirectory,
    setPublicGuildDirectory,
    publicGuildJoinStatusByGuildId,
    setPublicGuildJoinStatusByGuildId,
    publicGuildJoinErrorByGuildId,
    setPublicGuildJoinErrorByGuildId,
    searchQuery,
    setSearchQuery,
    searchError,
    setSearchError,
    isSearching,
    setSearching,
    searchResults,
    setSearchResults,
    isRunningSearchOps,
    setRunningSearchOps,
    searchOpsStatus,
    setSearchOpsStatus,
  };
}

export type WorkspaceChannelState = ReturnType<typeof createWorkspaceChannelState>;
export type FriendshipsState = ReturnType<typeof createFriendshipsState>;
export type DiscoveryState = ReturnType<typeof createDiscoveryState>;

export function createWorkspaceState() {
  const workspaceChannel = createWorkspaceChannelState();
  const friendships = createFriendshipsState();
  const discovery = createDiscoveryState();

  return {
    workspaceChannel,
    friendships,
    discovery,
  };
}

export type WorkspaceState = ReturnType<typeof createWorkspaceState>;
