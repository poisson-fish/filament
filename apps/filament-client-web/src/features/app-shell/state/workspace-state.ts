import { createSignal } from "solid-js";
import type {
  ChannelId,
  ChannelKindName,
  ChannelPermissionSnapshot,
  FriendRecord,
  FriendRequestList,
  GuildId,
  GuildRecord,
  GuildVisibility,
  SearchResults,
  WorkspaceRecord,
} from "../../../domain/chat";
import type { PublicDirectoryJoinStatus } from "../types";

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
