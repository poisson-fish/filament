import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
  untrack,
} from "solid-js";
import type { AuthSession } from "../domain/auth";
import { DomainValidationError } from "../domain/auth";
import {
  attachmentFilenameFromInput,
  channelNameFromInput,
  guildVisibilityFromInput,
  guildNameFromInput,
  messageContentFromInput,
  permissionFromInput,
  reactionEmojiFromInput,
  roleFromInput,
  searchQueryFromInput,
  userIdFromInput,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelId,
  type ChannelPermissionSnapshot,
  type FriendRecord,
  type FriendRequestList,
  type GuildVisibility,
  type GuildId,
  type MarkdownToken,
  type MessageId,
  type MessageRecord,
  type GuildRecord,
  type PermissionName,
  type RoleName,
  type SearchResults,
  type UserId,
  type WorkspaceRecord,
} from "../domain/chat";
import {
  ApiError,
  addGuildMember,
  acceptFriendRequest,
  addMessageReaction,
  banGuildMember,
  createChannel,
  createChannelMessage,
  createFriendRequest,
  createGuild,
  deleteFriendRequest,
  deleteChannelAttachment,
  deleteChannelMessage,
  downloadChannelAttachment,
  editChannelMessage,
  echoMessage,
  fetchChannelMessages,
  fetchChannelPermissionSnapshot,
  fetchFriendRequests,
  fetchFriends,
  fetchHealth,
  fetchMe,
  fetchPublicGuildDirectory,
  issueVoiceToken,
  kickGuildMember,
  logoutAuthSession,
  rebuildGuildSearchIndex,
  reconcileGuildSearchIndex,
  refreshAuthSession,
  removeMessageReaction,
  removeFriend,
  searchGuildMessages,
  setChannelRoleOverride,
  updateGuildMemberRole,
  uploadChannelAttachment,
} from "../lib/api";
import { useAuth } from "../lib/auth-context";
import { connectGateway } from "../lib/gateway";
import { clearWorkspaceCache, loadWorkspaceCache, saveWorkspaceCache } from "../lib/workspace-cache";
import {
  clearUsernameLookupCache,
  primeUsernameCache,
  resolveUsernames,
} from "../lib/username-cache";

const THUMBS_UP = reactionEmojiFromInput("👍");

interface ReactionView {
  count: number;
  reacted: boolean;
}

function reactionKey(messageId: MessageId, emoji: string): string {
  return `${messageId}|${emoji}`;
}

function channelKey(guildId: GuildId, channelId: ChannelId): string {
  return `${guildId}|${channelId}`;
}

function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  const kib = value / 1024;
  if (kib < 1024) {
    return `${kib.toFixed(1)} KiB`;
  }
  return `${(kib / 1024).toFixed(2)} MiB`;
}

function mapError(error: unknown, fallback: string): string {
  if (error instanceof DomainValidationError) {
    return error.message;
  }
  if (error instanceof ApiError) {
    if (error.code === "rate_limited") {
      return "Rate limited. Please wait and retry.";
    }
    if (error.code === "forbidden") {
      return "Permission denied for this action.";
    }
    if (error.code === "not_found") {
      return "Requested resource was not found.";
    }
    if (error.code === "network_error") {
      return "Cannot reach server. Verify API origin and TLS setup.";
    }
    if (error.code === "payload_too_large") {
      return "Payload is too large for this endpoint.";
    }
    if (error.code === "quota_exceeded") {
      return "Attachment quota exceeded for this user.";
    }
    if (error.code === "guild_creation_limit_reached") {
      return "Guild creation limit reached for this account.";
    }
    if (error.code === "invalid_credentials") {
      return "Authentication failed. Please login again.";
    }
    if (error.code === "invalid_request") {
      return "Request payload did not pass API validation.";
    }
    return `Request failed (${error.code}).`;
  }
  return fallback;
}

function profileErrorMessage(error: unknown): string {
  if (error instanceof ApiError && error.code === "invalid_credentials") {
    return "Session expired. Please login again.";
  }
  return mapError(error, "Profile unavailable.");
}

function formatMessageTime(createdAtUnix: number): string {
  return new Date(createdAtUnix * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function shortActor(value: string): string {
  return value.length > 14 ? `${value.slice(0, 14)}...` : value;
}

function upsertWorkspace(
  existing: WorkspaceRecord[],
  guildId: GuildId,
  updater: (workspace: WorkspaceRecord) => WorkspaceRecord,
): WorkspaceRecord[] {
  return existing.map((workspace) => (workspace.guildId === guildId ? updater(workspace) : workspace));
}

function mergeMessage(existing: MessageRecord[], incoming: MessageRecord): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === incoming.messageId);
  if (index >= 0) {
    const next = [...existing];
    next[index] = incoming;
    return next;
  }
  return [...existing, incoming];
}

function prependOlderMessages(
  existing: MessageRecord[],
  olderAscending: MessageRecord[],
): MessageRecord[] {
  const known = new Set(existing.map((entry) => entry.messageId));
  const prepend = olderAscending.filter((entry) => !known.has(entry.messageId));
  return [...prepend, ...existing];
}

function tokenizeToDisplayText(tokens: MarkdownToken[]): string {
  let output = "";
  let pendingLink: string | null = null;

  for (const token of tokens) {
    if (token.type === "text") {
      output += token.text;
      continue;
    }
    if (token.type === "code") {
      output += `\`${token.code}\``;
      continue;
    }
    if (token.type === "soft_break" || token.type === "hard_break") {
      output += "\n";
      continue;
    }
    if (token.type === "paragraph_end") {
      output += "\n\n";
      continue;
    }
    if (token.type === "list_item_start") {
      output += "• ";
      continue;
    }
    if (token.type === "list_item_end") {
      output += "\n";
      continue;
    }
    if (token.type === "link_start") {
      pendingLink = token.href;
      continue;
    }
    if (token.type === "link_end") {
      if (pendingLink) {
        output += ` (${pendingLink})`;
      }
      pendingLink = null;
    }
  }

  return output.trimEnd();
}

function parsePermissionCsv(value: string): PermissionName[] {
  const unique = new Set<PermissionName>();
  const tokens = value
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
  for (const token of tokens) {
    unique.add(permissionFromInput(token));
  }
  return [...unique];
}

const MAX_CACHED_WORKSPACES = 64;
const MAX_CHANNEL_PROBES_PER_WORKSPACE = 16;

async function canAccessChannel(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
): Promise<boolean> {
  try {
    const snapshot = await fetchChannelPermissionSnapshot(session, guildId, channelId);
    return snapshot.permissions.includes("create_message");
  } catch (error) {
    if (error instanceof ApiError) {
      if (
        error.code === "forbidden" ||
        error.code === "not_found" ||
        error.code === "invalid_credentials" ||
        error.code === "network_error"
      ) {
        return false;
      }
    }
    return false;
  }
}

function canDiscoverWorkspaceOperation(
  role: RoleName | undefined,
): boolean {
  return role === "owner" || role === "moderator";
}

export function AppShellPage() {
  const auth = useAuth();

  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
  const [activeGuildId, setActiveGuildId] = createSignal<GuildId | null>(null);
  const [activeChannelId, setActiveChannelId] = createSignal<ChannelId | null>(null);
  const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);

  const [composer, setComposer] = createSignal("");
  const [messageStatus, setMessageStatus] = createSignal("");
  const [messageError, setMessageError] = createSignal("");
  const [isLoadingMessages, setLoadingMessages] = createSignal(false);
  const [isLoadingOlder, setLoadingOlder] = createSignal(false);
  const [isSendingMessage, setSendingMessage] = createSignal(false);
  const [messages, setMessages] = createSignal<MessageRecord[]>([]);
  const [nextBefore, setNextBefore] = createSignal<MessageId | null>(null);
  const [reactionState, setReactionState] = createSignal<Record<string, ReactionView>>({});
  const [editingMessageId, setEditingMessageId] = createSignal<MessageId | null>(null);
  const [editingDraft, setEditingDraft] = createSignal("");
  const [isSavingEdit, setSavingEdit] = createSignal(false);
  const [deletingMessageId, setDeletingMessageId] = createSignal<MessageId | null>(null);

  const [createGuildName, setCreateGuildName] = createSignal("Security Ops");
  const [createGuildVisibility, setCreateGuildVisibility] = createSignal<GuildVisibility>("private");
  const [createChannelName, setCreateChannelName] = createSignal("incident-room");
  const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
  const [workspaceError, setWorkspaceError] = createSignal("");
  const [showWorkspaceCreateForm, setShowWorkspaceCreateForm] = createSignal(false);
  const [publicGuildSearchQuery, setPublicGuildSearchQuery] = createSignal("");
  const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
  const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
  const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<GuildRecord[]>([]);
  const [friendRecipientUserIdInput, setFriendRecipientUserIdInput] = createSignal("");
  const [friends, setFriends] = createSignal<FriendRecord[]>([]);
  const [friendRequests, setFriendRequests] = createSignal<FriendRequestList>({
    incoming: [],
    outgoing: [],
  });
  const [isRunningFriendAction, setRunningFriendAction] = createSignal(false);
  const [friendStatus, setFriendStatus] = createSignal("");
  const [friendError, setFriendError] = createSignal("");

  const [newChannelName, setNewChannelName] = createSignal("backend");
  const [isCreatingChannel, setCreatingChannel] = createSignal(false);
  const [channelCreateError, setChannelCreateError] = createSignal("");
  const [showNewChannelForm, setShowNewChannelForm] = createSignal(false);

  const [searchQuery, setSearchQuery] = createSignal("");
  const [searchError, setSearchError] = createSignal("");
  const [isSearching, setSearching] = createSignal(false);
  const [searchResults, setSearchResults] = createSignal<SearchResults | null>(null);
  const [isRunningSearchOps, setRunningSearchOps] = createSignal(false);
  const [searchOpsStatus, setSearchOpsStatus] = createSignal("");

  const [gatewayOnline, setGatewayOnline] = createSignal(false);
  const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);
  const [resolvedUsernames, setResolvedUsernames] = createSignal<Record<string, string>>({});

  const [attachmentByChannel, setAttachmentByChannel] = createSignal<Record<string, AttachmentRecord[]>>({});
  const [selectedAttachment, setSelectedAttachment] = createSignal<File | null>(null);
  const [attachmentFilename, setAttachmentFilename] = createSignal("");
  const [attachmentStatus, setAttachmentStatus] = createSignal("");
  const [attachmentError, setAttachmentError] = createSignal("");
  const [isUploadingAttachment, setUploadingAttachment] = createSignal(false);
  const [downloadingAttachmentId, setDownloadingAttachmentId] = createSignal<AttachmentId | null>(null);
  const [deletingAttachmentId, setDeletingAttachmentId] = createSignal<AttachmentId | null>(null);

  const [voiceCanPublish, setVoiceCanPublish] = createSignal(true);
  const [voiceCanSubscribe, setVoiceCanSubscribe] = createSignal(false);
  const [voiceMicrophone, setVoiceMicrophone] = createSignal(true);
  const [voiceCamera, setVoiceCamera] = createSignal(false);
  const [voiceScreenShare, setVoiceScreenShare] = createSignal(false);
  const [isIssuingVoiceToken, setIssuingVoiceToken] = createSignal(false);
  const [voiceTokenStatus, setVoiceTokenStatus] = createSignal("");
  const [voiceTokenError, setVoiceTokenError] = createSignal("");
  const [voiceTokenPreview, setVoiceTokenPreview] = createSignal<string | null>(null);

  const [moderationUserIdInput, setModerationUserIdInput] = createSignal("");
  const [moderationRoleInput, setModerationRoleInput] = createSignal<RoleName>("member");
  const [isModerating, setModerating] = createSignal(false);
  const [moderationStatus, setModerationStatus] = createSignal("");
  const [moderationError, setModerationError] = createSignal("");

  const [overrideRoleInput, setOverrideRoleInput] = createSignal<RoleName>("member");
  const [overrideAllowCsv, setOverrideAllowCsv] = createSignal("create_message");
  const [overrideDenyCsv, setOverrideDenyCsv] = createSignal("");

  const [isRefreshingSession, setRefreshingSession] = createSignal(false);
  const [sessionStatus, setSessionStatus] = createSignal("");
  const [sessionError, setSessionError] = createSignal("");
  const [channelPermissions, setChannelPermissions] = createSignal<ChannelPermissionSnapshot | null>(null);

  const [healthStatus, setHealthStatus] = createSignal("");
  const [echoInput, setEchoInput] = createSignal("hello filament");
  const [diagError, setDiagError] = createSignal("");
  const [isCheckingHealth, setCheckingHealth] = createSignal(false);
  const [isEchoing, setEchoing] = createSignal(false);

  const activeWorkspace = createMemo(
    () => workspaces().find((workspace) => workspace.guildId === activeGuildId()) ?? null,
  );

  const activeChannel = createMemo(
    () =>
      activeWorkspace()?.channels.find((channel) => channel.channelId === activeChannelId()) ??
      null,
  );

  const hasPermission = (permission: PermissionName): boolean =>
    channelPermissions()?.permissions.includes(permission) ?? false;

  const canAccessActiveChannel = createMemo(() => hasPermission("create_message"));
  const canManageWorkspaceChannels = createMemo(() => {
    const role = channelPermissions()?.role;
    return canDiscoverWorkspaceOperation(role);
  });
  const canManageSearchMaintenance = createMemo(() => canManageWorkspaceChannels());
  const canManageRoles = createMemo(() => hasPermission("manage_roles"));
  const canManageChannelOverrides = createMemo(() => hasPermission("manage_channel_overrides"));
  const canBanMembers = createMemo(() => hasPermission("ban_member"));
  const canDeleteMessages = createMemo(() => hasPermission("delete_message"));
  const hasModerationAccess = createMemo(
    () => canManageRoles() || canBanMembers() || canManageChannelOverrides(),
  );
  const canDismissWorkspaceCreateForm = createMemo(() => workspaces().length > 0);

  const activeChannelKey = createMemo(() => {
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    return guildId && channelId ? channelKey(guildId, channelId) : null;
  });

  const activeAttachments = createMemo(() => {
    const key = activeChannelKey();
    if (!key) {
      return [];
    }
    return attachmentByChannel()[key] ?? [];
  });

  const displayUserLabel = (userId: string): string => resolvedUsernames()[userId] ?? shortActor(userId);

  const loadPublicGuildDirectory = async (query?: string) => {
    const session = auth.session();
    if (!session) {
      setPublicGuildDirectory([]);
      return;
    }
    if (isSearchingPublicGuilds()) {
      return;
    }
    setSearchingPublicGuilds(true);
    setPublicGuildSearchError("");
    try {
      const directory = await fetchPublicGuildDirectory(session, {
        query,
        limit: 20,
      });
      setPublicGuildDirectory(directory.guilds);
    } catch (error) {
      setPublicGuildSearchError(mapError(error, "Unable to load public workspace directory."));
      setPublicGuildDirectory([]);
    } finally {
      setSearchingPublicGuilds(false);
    }
  };

  const refreshFriendDirectory = async () => {
    const session = auth.session();
    if (!session) {
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      return;
    }
    setFriendError("");
    try {
      const [friendList, requestList] = await Promise.all([
        fetchFriends(session),
        fetchFriendRequests(session),
      ]);
      setFriends(friendList);
      setFriendRequests(requestList);
    } catch (error) {
      setFriendError(mapError(error, "Unable to load friendship state."));
    }
  };

  const [profile] = createResource(async () => {
    const session = auth.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return fetchMe(session);
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      setWorkspaces([]);
      setChannelPermissions(null);
      setWorkspaceBootstrapDone(true);
      return;
    }

    let cancelled = false;
    setWorkspaceBootstrapDone(false);
    const cached = loadWorkspaceCache().slice(0, MAX_CACHED_WORKSPACES);

    const bootstrap = async () => {
      const validated = await Promise.all(
        cached.map(async (workspace) => {
          const sampledChannels = workspace.channels.slice(0, MAX_CHANNEL_PROBES_PER_WORKSPACE);
          if (sampledChannels.length === 0) {
            return null;
          }

          const channelAccess = await Promise.all(
            sampledChannels.map((channel) =>
              canAccessChannel(session, workspace.guildId, channel.channelId),
            ),
          );
          const channels = sampledChannels.filter((_, index) => channelAccess[index]);
          if (channels.length === 0) {
            return null;
          }

          return {
            ...workspace,
            channels,
          };
        }),
      );

      if (cancelled) {
        return;
      }
      const filtered = validated.filter((entry): entry is WorkspaceRecord => entry !== null);
      setWorkspaces(filtered);
      const selectedGuild = activeGuildId();
      const selectedWorkspace =
        (selectedGuild && filtered.find((workspace) => workspace.guildId === selectedGuild)) ??
        filtered[0] ??
        null;
      setActiveGuildId(selectedWorkspace?.guildId ?? null);
      const selectedChannel = activeChannelId();
      const nextChannel =
        (selectedChannel &&
          selectedWorkspace?.channels.find((channel) => channel.channelId === selectedChannel)) ??
        selectedWorkspace?.channels[0] ??
        null;
      setActiveChannelId(nextChannel?.channelId ?? null);
      setWorkspaceBootstrapDone(true);
    };

    void bootstrap();

    onCleanup(() => {
      cancelled = true;
    });
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setPublicGuildDirectory([]);
      setPublicGuildSearchError("");
      return;
    }
    void untrack(() => loadPublicGuildDirectory());
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      setFriendStatus("");
      setFriendError("");
      return;
    }
    void untrack(() => refreshFriendDirectory());
  });

  createEffect(() => {
    const value = profile();
    if (!value) {
      return;
    }
    primeUsernameCache([{ userId: value.userId, username: value.username }]);
    setResolvedUsernames((existing) => ({
      ...existing,
      [value.userId]: value.username,
    }));
  });

  createEffect(() => {
    const known = [
      ...friends().map((friend) => ({
        userId: friend.userId,
        username: friend.username,
      })),
      ...friendRequests().incoming.map((request) => ({
        userId: request.senderUserId,
        username: request.senderUsername,
      })),
      ...friendRequests().outgoing.map((request) => ({
        userId: request.recipientUserId,
        username: request.recipientUsername,
      })),
    ];
    if (known.length === 0) {
      return;
    }
    primeUsernameCache(known);
    setResolvedUsernames((existing) => ({
      ...existing,
      ...Object.fromEntries(known.map((entry) => [entry.userId, entry.username])),
    }));
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      return;
    }

    const lookupIds = new Set<UserId>();
    for (const message of messages()) {
      lookupIds.add(message.authorId);
    }
    for (const memberId of onlineMembers()) {
      try {
        lookupIds.add(userIdFromInput(memberId));
      } catch {
        continue;
      }
    }
    const result = searchResults();
    if (result) {
      for (const message of result.messages) {
        lookupIds.add(message.authorId);
      }
    }
    if (lookupIds.size === 0) {
      return;
    }

    let cancelled = false;
    const resolveVisibleUsernames = async () => {
      try {
        const resolved = await resolveUsernames(session, [...lookupIds]);
        if (cancelled || Object.keys(resolved).length === 0) {
          return;
        }
        setResolvedUsernames((existing) => ({
          ...existing,
          ...resolved,
        }));
      } catch {
        // Keep user-id fallback rendering if lookup fails.
      }
    };
    void resolveVisibleUsernames();

    onCleanup(() => {
      cancelled = true;
    });
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(workspaces());
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    if (workspaces().length === 0) {
      setShowWorkspaceCreateForm(true);
    }
  });

  createEffect(() => {
    const selectedGuild = activeGuildId();
    if (!selectedGuild || !workspaces().some((workspace) => workspace.guildId === selectedGuild)) {
      setActiveGuildId(workspaces()[0]?.guildId ?? null);
      return;
    }

    const channel = activeChannelId();
    const workspace = workspaces().find((entry) => entry.guildId === selectedGuild);
    if (!workspace) {
      return;
    }
    if (!channel || !workspace.channels.some((entry) => entry.channelId === channel)) {
      setActiveChannelId(workspace.channels[0]?.channelId ?? null);
    }
  });

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setChannelPermissions(null);
      return;
    }

    let cancelled = false;
    const loadPermissions = async () => {
      try {
        const snapshot = await fetchChannelPermissionSnapshot(session, guildId, channelId);
        if (!cancelled) {
          setChannelPermissions(snapshot);
        }
      } catch (error) {
        if (!cancelled) {
          setChannelPermissions(null);
          if (error instanceof ApiError && (error.code === "forbidden" || error.code === "not_found")) {
            setWorkspaces((existing) =>
              existing
                .map((workspace) => {
                  if (workspace.guildId !== guildId) {
                    return workspace;
                  }
                  return {
                    ...workspace,
                    channels: workspace.channels.filter((channel) => channel.channelId !== channelId),
                  };
                })
                .filter((workspace) => workspace.channels.length > 0),
            );
          }
        }
      }
    };
    void loadPermissions();

    onCleanup(() => {
      cancelled = true;
    });
  });

  const refreshMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessages([]);
      setNextBefore(null);
      return;
    }

    setMessageError("");
    setLoadingMessages(true);
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, { limit: 50 });
      setMessages([...history.messages].reverse());
      setNextBefore(history.nextBefore);
      setEditingMessageId(null);
      setEditingDraft("");
    } catch (error) {
      setMessageError(mapError(error, "Unable to load messages."));
      setMessages([]);
      setNextBefore(null);
    } finally {
      setLoadingMessages(false);
    }
  };

  const loadOlderMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const before = nextBefore();
    if (!session || !guildId || !channelId || !before || isLoadingOlder()) {
      return;
    }

    setLoadingOlder(true);
    setMessageError("");
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, {
        limit: 50,
        before,
      });
      const olderAscending = [...history.messages].reverse();
      setMessages((existing) => prependOlderMessages(existing, olderAscending));
      setNextBefore(history.nextBefore);
    } catch (error) {
      setMessageError(mapError(error, "Unable to load older messages."));
    } finally {
      setLoadingOlder(false);
    }
  };

  createEffect(() => {
    void activeGuildId();
    void activeChannelId();
    const canRead = canAccessActiveChannel();
    setReactionState({});
    setSearchResults(null);
    setSearchError("");
    setSearchOpsStatus("");
    setAttachmentStatus("");
    setAttachmentError("");
    setVoiceTokenStatus("");
    setVoiceTokenError("");
    setVoiceTokenPreview(null);
    if (canRead) {
      void refreshMessages();
    } else {
      setMessages([]);
      setNextBefore(null);
    }
  });

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || !canAccessActiveChannel()) {
      setGatewayOnline(false);
      setOnlineMembers([]);
      return;
    }

    const gateway = connectGateway(session.accessToken, guildId, channelId, {
      onOpenStateChange: (isOpen) => setGatewayOnline(isOpen),
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        setMessages((existing) => mergeMessage(existing, message));
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers((existing) => {
          if (payload.status === "online") {
            return existing.includes(payload.userId) ? existing : [...existing, payload.userId];
          }
          return existing.filter((entry) => entry !== payload.userId);
        });
      },
    });

    onCleanup(() => gateway.close());
  });

  const createWorkspace = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session) {
      setWorkspaceError("Missing auth session.");
      return;
    }
    if (isCreatingWorkspace()) {
      return;
    }

    setWorkspaceError("");
    setCreatingWorkspace(true);
    try {
      const hadWorkspace = workspaces().length > 0;
      const guild = await createGuild(session, {
        name: guildNameFromInput(createGuildName()),
        visibility: guildVisibilityFromInput(createGuildVisibility()),
      });
      const channel = await createChannel(session, guild.guildId, {
        name: channelNameFromInput(createChannelName()),
      });
      const createdWorkspace: WorkspaceRecord = {
        guildId: guild.guildId,
        guildName: guild.name,
        visibility: guild.visibility,
        channels: [channel],
      };
      setWorkspaces((existing) => [...existing, createdWorkspace]);
      setActiveGuildId(createdWorkspace.guildId);
      setActiveChannelId(channel.channelId);
      setMessageStatus("Workspace created.");
      setShowWorkspaceCreateForm(!hadWorkspace);
    } catch (error) {
      setWorkspaceError(mapError(error, "Unable to create workspace."));
    } finally {
      setCreatingWorkspace(false);
    }
  };

  const createNewChannel = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setChannelCreateError("Select a workspace first.");
      return;
    }
    if (isCreatingChannel()) {
      return;
    }

    setChannelCreateError("");
    setCreatingChannel(true);
    try {
      const created = await createChannel(session, guildId, {
        name: channelNameFromInput(newChannelName()),
      });
      setWorkspaces((existing) =>
        upsertWorkspace(existing, guildId, (workspace) => {
          if (workspace.channels.some((channel) => channel.channelId === created.channelId)) {
            return workspace;
          }
          return {
            ...workspace,
            channels: [...workspace.channels, created],
          };
        }),
      );
      setActiveChannelId(created.channelId);
      setShowNewChannelForm(false);
      setNewChannelName("backend");
      setMessageStatus("Channel created.");
    } catch (error) {
      setChannelCreateError(mapError(error, "Unable to create channel."));
    } finally {
      setCreatingChannel(false);
    }
  };

  const sendMessage = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessageError("Select a channel first.");
      return;
    }

    if (isSendingMessage()) {
      return;
    }

    setMessageError("");
    setMessageStatus("");
    setSendingMessage(true);
    try {
      const created = await createChannelMessage(session, guildId, channelId, {
        content: messageContentFromInput(composer()),
      });
      setMessages((existing) => mergeMessage(existing, created));
      setComposer("");
    } catch (error) {
      setMessageError(mapError(error, "Unable to send message."));
    } finally {
      setSendingMessage(false);
    }
  };

  const beginEditMessage = (message: MessageRecord) => {
    setEditingMessageId(message.messageId);
    setEditingDraft(message.content);
  };

  const cancelEditMessage = () => {
    setEditingMessageId(null);
    setEditingDraft("");
  };

  const saveEditMessage = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || isSavingEdit()) {
      return;
    }

    setSavingEdit(true);
    setMessageError("");
    try {
      const updated = await editChannelMessage(session, guildId, channelId, messageId, {
        content: messageContentFromInput(editingDraft()),
      });
      setMessages((existing) => mergeMessage(existing, updated));
      setEditingMessageId(null);
      setEditingDraft("");
      setMessageStatus("Message updated.");
    } catch (error) {
      setMessageError(mapError(error, "Unable to edit message."));
    } finally {
      setSavingEdit(false);
    }
  };

  const removeMessage = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || deletingMessageId()) {
      return;
    }

    setDeletingMessageId(messageId);
    setMessageError("");
    try {
      await deleteChannelMessage(session, guildId, channelId, messageId);
      setMessages((existing) => existing.filter((entry) => entry.messageId !== messageId));
      if (editingMessageId() === messageId) {
        cancelEditMessage();
      }
      setMessageStatus("Message deleted.");
    } catch (error) {
      setMessageError(mapError(error, "Unable to delete message."));
    } finally {
      setDeletingMessageId(null);
    }
  };

  const toggleThumbsUp = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, THUMBS_UP);
    const state = reactionState()[key] ?? { count: 0, reacted: false };

    try {
      if (state.reacted) {
        const response = await removeMessageReaction(session, guildId, channelId, messageId, THUMBS_UP);
        setReactionState((existing) => ({
          ...existing,
          [key]: { count: response.count, reacted: false },
        }));
      } else {
        const response = await addMessageReaction(session, guildId, channelId, messageId, THUMBS_UP);
        setReactionState((existing) => ({
          ...existing,
          [key]: { count: response.count, reacted: true },
        }));
      }
    } catch (error) {
      setMessageError(mapError(error, "Unable to update reaction."));
    }
  };

  const runSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setSearchError("Select a workspace first.");
      return;
    }

    if (isSearching()) {
      return;
    }

    setSearching(true);
    setSearchError("");
    try {
      const results = await searchGuildMessages(session, guildId, {
        query: searchQueryFromInput(searchQuery()),
        limit: 20,
        channelId: activeChannelId() ?? undefined,
      });
      setSearchResults(results);
    } catch (error) {
      setSearchError(mapError(error, "Search request failed."));
      setSearchResults(null);
    } finally {
      setSearching(false);
    }
  };

  const runPublicGuildSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    await loadPublicGuildDirectory(publicGuildSearchQuery());
  };

  const submitFriendRequest = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      const recipientUserId = userIdFromInput(friendRecipientUserIdInput().trim());
      await createFriendRequest(session, recipientUserId);
      setFriendRecipientUserIdInput("");
      await refreshFriendDirectory();
      setFriendStatus("Friend request sent.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to create friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const acceptIncomingFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await acceptFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request accepted.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to accept friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const dismissFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await deleteFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const removeFriendship = async (friendUserId: UserId) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await removeFriend(session, friendUserId);
      await refreshFriendDirectory();
      setFriendStatus("Friend removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const rebuildSearch = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isRunningSearchOps()) {
      return;
    }

    setRunningSearchOps(true);
    setSearchError("");
    setSearchOpsStatus("");
    try {
      await rebuildGuildSearchIndex(session, guildId);
      setSearchOpsStatus("Search index rebuild queued.");
    } catch (error) {
      setSearchError(mapError(error, "Unable to rebuild search index."));
    } finally {
      setRunningSearchOps(false);
    }
  };

  const reconcileSearch = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isRunningSearchOps()) {
      return;
    }

    setRunningSearchOps(true);
    setSearchError("");
    setSearchOpsStatus("");
    try {
      const result = await reconcileGuildSearchIndex(session, guildId);
      setSearchOpsStatus(`Reconciled search index (upserted ${result.upserted}, deleted ${result.deleted}).`);
    } catch (error) {
      setSearchError(mapError(error, "Unable to reconcile search index."));
    } finally {
      setRunningSearchOps(false);
    }
  };

  const uploadAttachment = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const file = selectedAttachment();
    if (!session || !guildId || !channelId) {
      setAttachmentError("Select a channel first.");
      return;
    }
    if (!file) {
      setAttachmentError("Select a file to upload.");
      return;
    }
    if (isUploadingAttachment()) {
      return;
    }

    setAttachmentStatus("");
    setAttachmentError("");
    setUploadingAttachment(true);

    try {
      const filename = attachmentFilenameFromInput(
        attachmentFilename().trim().length > 0 ? attachmentFilename().trim() : file.name,
      );
      const uploaded = await uploadChannelAttachment(session, guildId, channelId, file, filename);
      const key = channelKey(guildId, channelId);
      setAttachmentByChannel((existing) => {
        const current = existing[key] ?? [];
        const deduped = current.filter((entry) => entry.attachmentId !== uploaded.attachmentId);
        return {
          ...existing,
          [key]: [uploaded, ...deduped],
        };
      });
      setAttachmentStatus(`Uploaded ${uploaded.filename} (${formatBytes(uploaded.sizeBytes)}).`);
      setSelectedAttachment(null);
      setAttachmentFilename("");
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to upload attachment."));
    } finally {
      setUploadingAttachment(false);
    }
  };

  const downloadAttachment = async (record: AttachmentRecord) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || downloadingAttachmentId()) {
      return;
    }

    setDownloadingAttachmentId(record.attachmentId);
    setAttachmentError("");
    try {
      const payload = await downloadChannelAttachment(session, guildId, channelId, record.attachmentId);
      const blob = new Blob([payload.bytes], {
        type: payload.mimeType ?? record.mimeType,
      });
      const objectUrl = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = objectUrl;
      anchor.download = record.filename;
      anchor.rel = "noopener";
      anchor.click();
      window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to download attachment."));
    } finally {
      setDownloadingAttachmentId(null);
    }
  };

  const removeAttachment = async (record: AttachmentRecord) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || deletingAttachmentId()) {
      return;
    }

    setDeletingAttachmentId(record.attachmentId);
    setAttachmentError("");
    try {
      await deleteChannelAttachment(session, guildId, channelId, record.attachmentId);
      const key = channelKey(guildId, channelId);
      setAttachmentByChannel((existing) => ({
        ...existing,
        [key]: (existing[key] ?? []).filter((entry) => entry.attachmentId !== record.attachmentId),
      }));
      setAttachmentStatus(`Deleted ${record.filename}.`);
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to delete attachment."));
    } finally {
      setDeletingAttachmentId(null);
    }
  };

  const requestVoiceToken = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || isIssuingVoiceToken()) {
      return;
    }

    const publishSources = [] as Array<"microphone" | "camera" | "screen_share">;
    if (voiceMicrophone()) {
      publishSources.push("microphone");
    }
    if (voiceCamera()) {
      publishSources.push("camera");
    }
    if (voiceScreenShare()) {
      publishSources.push("screen_share");
    }

    setIssuingVoiceToken(true);
    setVoiceTokenError("");
    setVoiceTokenStatus("");
    try {
      const token = await issueVoiceToken(session, guildId, channelId, {
        canPublish: voiceCanPublish(),
        canSubscribe: voiceCanSubscribe(),
        publishSources,
      });
      setVoiceTokenPreview(token.token.slice(0, 18));
      setVoiceTokenStatus(
        `Voice token issued (${token.expiresInSecs}s, publish=${token.canPublish}, subscribe=${token.canSubscribe}).`,
      );
    } catch (error) {
      setVoiceTokenError(mapError(error, "Unable to issue voice token."));
      setVoiceTokenPreview(null);
    } finally {
      setIssuingVoiceToken(false);
    }
  };

  const runModerationAction = async (
    action: (sessionUserId: UserId, sessionUsername: string) => Promise<void>,
  ) => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isModerating()) {
      return;
    }

    setModerationError("");
    setModerationStatus("");
    setModerating(true);
    try {
      const me = await fetchMe(session);
      await action(me.userId, me.username);
    } catch (error) {
      setModerationError(mapError(error, "Moderation action failed."));
    } finally {
      setModerating(false);
    }
  };

  const runMemberAction = async (action: "add" | "role" | "kick" | "ban") => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setModerationError("Select a workspace first.");
      return;
    }

    let targetUserId: UserId;
    try {
      targetUserId = userIdFromInput(moderationUserIdInput().trim());
    } catch (error) {
      setModerationError(mapError(error, "Target user ID is invalid."));
      return;
    }
    await runModerationAction(async () => {
      if (action === "add") {
        await addGuildMember(session, guildId, targetUserId);
        setModerationStatus("Member add request accepted.");
        return;
      }
      if (action === "role") {
        const role = roleFromInput(moderationRoleInput());
        await updateGuildMemberRole(session, guildId, targetUserId, role);
        setModerationStatus(`Member role updated to ${role}.`);
        return;
      }
      if (action === "kick") {
        await kickGuildMember(session, guildId, targetUserId);
        setModerationStatus("Member kicked.");
        return;
      }
      await banGuildMember(session, guildId, targetUserId);
      setModerationStatus("Member banned.");
    });
  };

  const applyOverride = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || isModerating()) {
      return;
    }

    try {
      const allow = parsePermissionCsv(overrideAllowCsv());
      const deny = parsePermissionCsv(overrideDenyCsv());
      if (allow.some((permission) => deny.includes(permission))) {
        throw new DomainValidationError("Allow and deny permission sets cannot overlap.");
      }

      setModerating(true);
      setModerationError("");
      setModerationStatus("");
      await setChannelRoleOverride(session, guildId, channelId, roleFromInput(overrideRoleInput()), {
        allow,
        deny,
      });
      setModerationStatus("Channel role override updated.");
    } catch (error) {
      setModerationError(mapError(error, "Unable to set channel override."));
    } finally {
      setModerating(false);
    }
  };

  const refreshSession = async () => {
    const session = auth.session();
    if (!session || isRefreshingSession()) {
      return;
    }

    setRefreshingSession(true);
    setSessionError("");
    setSessionStatus("");
    try {
      const next = await refreshAuthSession(session.refreshToken);
      auth.setAuthenticatedSession(next);
      setSessionStatus("Session refreshed.");
    } catch (error) {
      setSessionError(mapError(error, "Unable to refresh session."));
    } finally {
      setRefreshingSession(false);
    }
  };

  const logout = async () => {
    const session = auth.session();
    if (session) {
      try {
        await logoutAuthSession(session.refreshToken);
      } catch {
        // best-effort logout; local session will still be cleared
      }
    }
    auth.clearAuthenticatedSession();
    clearWorkspaceCache();
  };

  const runHealthCheck = async () => {
    if (isCheckingHealth()) {
      return;
    }
    setCheckingHealth(true);
    setDiagError("");
    try {
      const health = await fetchHealth();
      setHealthStatus(`Health: ${health.status}`);
    } catch (error) {
      setDiagError(mapError(error, "Health check failed."));
    } finally {
      setCheckingHealth(false);
    }
  };

  const runEcho = async (event: SubmitEvent) => {
    event.preventDefault();
    if (isEchoing()) {
      return;
    }

    setEchoing(true);
    setDiagError("");
    try {
      const echoed = await echoMessage({ message: echoInput() });
      setHealthStatus(`Echo: ${echoed.slice(0, 60)}`);
    } catch (error) {
      setDiagError(mapError(error, "Echo request failed."));
    } finally {
      setEchoing(false);
    }
  };

  return (
    <div class="app-shell">
      <aside class="server-rail" aria-label="servers">
        <header class="rail-label">WS</header>
        <For each={workspaces()}>
          {(workspace) => (
            <button
              title={`${workspace.guildName} (${workspace.visibility})`}
              classList={{ active: activeGuildId() === workspace.guildId }}
              onClick={() => {
                setActiveGuildId(workspace.guildId);
                setActiveChannelId(workspace.channels[0]?.channelId ?? null);
              }}
            >
              {workspace.guildName.slice(0, 1).toUpperCase()}
            </button>
          )}
        </For>
      </aside>

      <aside class="channel-rail">
        <header>
          <h2>{activeWorkspace()?.guildName ?? "No Workspace"}</h2>
          <span>
            {activeWorkspace() ? `${activeWorkspace()!.visibility} workspace` : "Hardened workspace"}
          </span>
        </header>

        <Switch>
          <Match when={!activeWorkspace()}>
            <p class="muted">Create a workspace to begin.</p>
          </Match>
          <Match when={activeWorkspace()}>
            <nav aria-label="channels">
              <p class="group-label">TEXT CHANNELS</p>
              <For each={activeWorkspace()?.channels ?? []}>
                {(channel) => (
                  <button
                    classList={{ active: activeChannelId() === channel.channelId }}
                    onClick={() => setActiveChannelId(channel.channelId)}
                  >
                    <span>#{channel.name}</span>
                  </button>
                )}
              </For>

              <Show when={canManageWorkspaceChannels()}>
                <button class="create-channel-toggle" onClick={() => setShowNewChannelForm((v) => !v)}>
                  {showNewChannelForm() ? "Cancel" : "New channel"}
                </button>
              </Show>

              <Show when={showNewChannelForm() && canManageWorkspaceChannels()}>
                <form class="inline-form" onSubmit={createNewChannel}>
                  <label>
                    Channel name
                    <input
                      value={newChannelName()}
                      onInput={(event) => setNewChannelName(event.currentTarget.value)}
                      maxlength="64"
                    />
                  </label>
                  <button type="submit" disabled={isCreatingChannel()}>
                    {isCreatingChannel() ? "Creating..." : "Create"}
                  </button>
                </form>
                <Show when={channelCreateError()}>
                  <p class="status error">{channelCreateError()}</p>
                </Show>
              </Show>
            </nav>
          </Match>
        </Switch>

        <section class="public-directory" aria-label="public-workspace-directory">
          <p class="group-label">PUBLIC WORKSPACES</p>
          <form class="inline-form" onSubmit={runPublicGuildSearch}>
            <label>
              Search
              <input
                value={publicGuildSearchQuery()}
                onInput={(event) => setPublicGuildSearchQuery(event.currentTarget.value)}
                maxlength="64"
                placeholder="workspace name"
              />
            </label>
            <button type="submit" disabled={isSearchingPublicGuilds()}>
              {isSearchingPublicGuilds() ? "Searching..." : "Find public"}
            </button>
          </form>
          <Show when={publicGuildSearchError()}>
            <p class="status error">{publicGuildSearchError()}</p>
          </Show>
          <ul>
            <For each={publicGuildDirectory()}>
              {(guild) => (
                <li>
                  <span class="presence online" />
                  <div class="stacked-meta">
                    <span>{guild.name}</span>
                    <span class="muted mono">{guild.visibility}</span>
                  </div>
                </li>
              )}
            </For>
            <Show when={!isSearchingPublicGuilds() && publicGuildDirectory().length === 0}>
              <li>
                <span class="presence idle" />
                no-public-workspaces
              </li>
            </Show>
          </ul>
        </section>

        <section class="public-directory" aria-label="friendships">
          <p class="group-label">FRIENDS</p>
          <form class="inline-form" onSubmit={submitFriendRequest}>
            <label>
              User ID
              <input
                value={friendRecipientUserIdInput()}
                onInput={(event) => setFriendRecipientUserIdInput(event.currentTarget.value)}
                maxlength="26"
                placeholder="01ARZ3NDEKTSV4RRFFQ69G5FAV"
              />
            </label>
            <button type="submit" disabled={isRunningFriendAction()}>
              {isRunningFriendAction() ? "Submitting..." : "Send request"}
            </button>
          </form>
          <Show when={friendStatus()}>
            <p class="status ok">{friendStatus()}</p>
          </Show>
          <Show when={friendError()}>
            <p class="status error">{friendError()}</p>
          </Show>

          <p class="group-label">INCOMING</p>
          <ul>
            <For each={friendRequests().incoming}>
              {(request) => (
                <li>
                  <div class="stacked-meta">
                    <span>{request.senderUsername}</span>
                    <span class="muted mono">{request.senderUserId}</span>
                  </div>
                  <div class="button-row">
                    <button
                      type="button"
                      onClick={() => void acceptIncomingFriendRequest(request.requestId)}
                      disabled={isRunningFriendAction()}
                    >
                      Accept
                    </button>
                    <button
                      type="button"
                      onClick={() => void dismissFriendRequest(request.requestId)}
                      disabled={isRunningFriendAction()}
                    >
                      Ignore
                    </button>
                  </div>
                </li>
              )}
            </For>
            <Show when={friendRequests().incoming.length === 0}>
              <li class="muted">no-incoming-requests</li>
            </Show>
          </ul>

          <p class="group-label">OUTGOING</p>
          <ul>
            <For each={friendRequests().outgoing}>
              {(request) => (
                <li>
                  <div class="stacked-meta">
                    <span>{request.recipientUsername}</span>
                    <span class="muted mono">{request.recipientUserId}</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => void dismissFriendRequest(request.requestId)}
                    disabled={isRunningFriendAction()}
                  >
                    Cancel
                  </button>
                </li>
              )}
            </For>
            <Show when={friendRequests().outgoing.length === 0}>
              <li class="muted">no-outgoing-requests</li>
            </Show>
          </ul>

          <p class="group-label">FRIEND LIST</p>
          <ul>
            <For each={friends()}>
              {(friend) => (
                <li>
                  <div class="stacked-meta">
                    <span>{friend.username}</span>
                    <span class="muted mono">{friend.userId}</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => void removeFriendship(friend.userId)}
                    disabled={isRunningFriendAction()}
                  >
                    Remove
                  </button>
                </li>
              )}
            </For>
            <Show when={friends().length === 0}>
              <li class="muted">no-friends</li>
            </Show>
          </ul>
        </section>
      </aside>

      <main class="chat-panel">
        <header class="chat-header">
          <div>
            <h3>{activeChannel() ? `#${activeChannel()!.name}` : "#no-channel"}</h3>
            <p>Gateway {gatewayOnline() ? "connected" : "disconnected"}</p>
          </div>
          <div class="header-actions">
            <span classList={{ "gateway-badge": true, online: gatewayOnline() }}>
              {gatewayOnline() ? "Live" : "Offline"}
            </span>
            <button type="button" onClick={() => void refreshMessages()}>
              Refresh
            </button>
            <button
              type="button"
              onClick={() => {
                setWorkspaceError("");
                setShowWorkspaceCreateForm((visible) => !visible);
              }}
              disabled={isCreatingWorkspace()}
            >
              {showWorkspaceCreateForm() ? "Close workspace form" : "New workspace"}
            </button>
            <button type="button" onClick={() => void refreshSession()} disabled={isRefreshingSession()}>
              {isRefreshingSession() ? "Refreshing..." : "Refresh session"}
            </button>
            <button class="logout" onClick={() => void logout()}>
              Logout
            </button>
          </div>
        </header>

        <Show when={showWorkspaceCreateForm()}>
          <section class="workspace-create-panel">
            <h4>Create workspace</h4>
            <form class="inline-form" onSubmit={createWorkspace}>
              <label>
                Workspace name
                <input
                  value={createGuildName()}
                  onInput={(event) => setCreateGuildName(event.currentTarget.value)}
                  maxlength="64"
                />
              </label>
              <label>
                Visibility
                <select
                  value={createGuildVisibility()}
                  onChange={(event) =>
                    setCreateGuildVisibility(guildVisibilityFromInput(event.currentTarget.value))
                  }
                >
                  <option value="private">private</option>
                  <option value="public">public</option>
                </select>
              </label>
              <label>
                First channel
                <input
                  value={createChannelName()}
                  onInput={(event) => setCreateChannelName(event.currentTarget.value)}
                  maxlength="64"
                />
              </label>
              <div class="button-row">
                <button type="submit" disabled={isCreatingWorkspace()}>
                  {isCreatingWorkspace() ? "Creating..." : "Create workspace"}
                </button>
                <Show when={canDismissWorkspaceCreateForm()}>
                  <button
                    type="button"
                    onClick={() => {
                      setWorkspaceError("");
                      setShowWorkspaceCreateForm(false);
                    }}
                  >
                    Cancel
                  </button>
                </Show>
              </div>
            </form>
            <Show when={workspaceError()}>
              <p class="status error">{workspaceError()}</p>
            </Show>
          </section>
        </Show>

        <Show
          when={workspaceBootstrapDone() && workspaces().length === 0}
          fallback={
            <>
              <Show when={!workspaceBootstrapDone()}>
                <p class="panel-note">Validating workspace access...</p>
              </Show>
              <Show when={workspaceBootstrapDone()}>
                <Show when={isLoadingMessages()}>
                  <p class="panel-note">Loading messages...</p>
                </Show>
                <Show when={messageError()}>
                  <p class="status error panel-note">{messageError()}</p>
                </Show>
                <Show when={sessionStatus()}>
                  <p class="status ok panel-note">{sessionStatus()}</p>
                </Show>
                <Show when={sessionError()}>
                  <p class="status error panel-note">{sessionError()}</p>
                </Show>
                <Show when={activeChannel() && !canAccessActiveChannel()}>
                  <p class="status error panel-note">
                    Channel is not visible with your current default permissions.
                  </p>
                </Show>

                <section class="message-list" aria-live="polite">
                  <Show when={nextBefore()}>
                    <button type="button" class="load-older" onClick={() => void loadOlderMessages()} disabled={isLoadingOlder()}>
                      {isLoadingOlder() ? "Loading older..." : "Load older messages"}
                    </button>
                  </Show>

                  <For each={messages()}>
                    {(message) => {
                      const state =
                        () => reactionState()[reactionKey(message.messageId, THUMBS_UP)] ?? { count: 0, reacted: false };
                      const isEditing = () => editingMessageId() === message.messageId;
                      const canEditOrDelete =
                        () => profile()?.userId === message.authorId || canDeleteMessages();
                      return (
                        <article class="message-row">
                          <p>
                            <strong>{displayUserLabel(message.authorId)}</strong>
                            <span>{formatMessageTime(message.createdAtUnix)}</span>
                          </p>
                          <Show
                            when={isEditing()}
                            fallback={<p class="message-tokenized">{tokenizeToDisplayText(message.markdownTokens) || message.content}</p>}
                          >
                            <form
                              class="inline-form message-edit"
                              onSubmit={(event) => {
                                event.preventDefault();
                                void saveEditMessage(message.messageId);
                              }}
                            >
                              <input
                                value={editingDraft()}
                                onInput={(event) => setEditingDraft(event.currentTarget.value)}
                                maxlength="2000"
                              />
                              <div class="message-actions">
                                <button type="submit" disabled={isSavingEdit()}>
                                  {isSavingEdit() ? "Saving..." : "Save"}
                                </button>
                                <button type="button" onClick={cancelEditMessage}>
                                  Cancel
                                </button>
                              </div>
                            </form>
                          </Show>
                          <Show when={canEditOrDelete()}>
                            <div class="message-actions compact">
                              <button type="button" onClick={() => beginEditMessage(message)}>
                                Edit
                              </button>
                              <button
                                type="button"
                                onClick={() => void removeMessage(message.messageId)}
                                disabled={deletingMessageId() === message.messageId}
                              >
                                {deletingMessageId() === message.messageId ? "Deleting..." : "Delete"}
                              </button>
                            </div>
                          </Show>
                          <div class="reaction-row">
                            <button
                              type="button"
                              classList={{ reacted: state().reacted }}
                              onClick={() => void toggleThumbsUp(message.messageId)}
                            >
                              {THUMBS_UP} {state().count}
                            </button>
                          </div>
                        </article>
                      );
                    }}
                  </For>

                  <Show when={!isLoadingMessages() && messages().length === 0 && !messageError()}>
                    <p class="muted">No messages yet in this channel.</p>
                  </Show>
                </section>

                <form class="composer" onSubmit={sendMessage}>
                  <input
                    value={composer()}
                    onInput={(event) => setComposer(event.currentTarget.value)}
                    maxlength="2000"
                    placeholder={activeChannel() ? `Message #${activeChannel()!.name}` : "Select channel"}
                    disabled={!activeChannel() || isSendingMessage() || !canAccessActiveChannel()}
                  />
                  <button type="submit" disabled={!activeChannel() || isSendingMessage() || !canAccessActiveChannel()}>
                    {isSendingMessage() ? "Sending..." : "Send"}
                  </button>
                </form>
              </Show>
            </>
          }
        >
          <section class="empty-workspace">
            <h3>Create your first workspace</h3>
            <p class="muted">Use the workspace panel above to create your first guild and channel.</p>
          </section>
        </Show>

        <Show when={messageStatus()}>
          <p class="status ok panel-note">{messageStatus()}</p>
        </Show>
      </main>

      <aside class="member-rail">
        <header>
          <h4>Ops Console</h4>
        </header>

        <Show when={profile.loading}>
          <p class="muted">Loading profile...</p>
        </Show>
        <Show when={profile.error}>
          <p class="status error">{profileErrorMessage(profile.error)}</p>
        </Show>
        <Show when={profile()}>
          {(value) => (
            <div class="profile-card">
              <p class="label">Username</p>
              <p>{value().username}</p>
              <p class="label">User ID</p>
              <p class="mono">{value().userId}</p>
            </div>
          )}
        </Show>

        <Show when={activeWorkspace() && activeChannel() && !canAccessActiveChannel()}>
          <p class="muted">No authorized workspace/channel selected for operator actions.</p>
        </Show>

        <Show when={canAccessActiveChannel()}>
          <section class="member-group">
          <p class="group-label">ONLINE ({onlineMembers().length})</p>
          <ul>
            <For each={onlineMembers()}>
              {(memberId) => (
                <li>
                  <span class="presence online" />
                  {displayUserLabel(memberId)}
                </li>
              )}
            </For>
            <Show when={onlineMembers().length === 0}>
              <li>
                <span class="presence idle" />
                no-presence-yet
              </li>
            </Show>
          </ul>
          </section>
        </Show>

        <Show when={canAccessActiveChannel()}>
          <section class="member-group">
          <p class="group-label">SEARCH</p>
          <form class="inline-form" onSubmit={runSearch}>
            <label>
              Query
              <input
                value={searchQuery()}
                onInput={(event) => setSearchQuery(event.currentTarget.value)}
                maxlength="256"
                placeholder="needle"
              />
            </label>
            <button type="submit" disabled={isSearching() || !activeWorkspace()}>
              {isSearching() ? "Searching..." : "Search"}
            </button>
          </form>
          <Show when={canManageSearchMaintenance()}>
            <div class="button-row">
              <button type="button" onClick={() => void rebuildSearch()} disabled={isRunningSearchOps() || !activeWorkspace()}>
                Rebuild Index
              </button>
              <button type="button" onClick={() => void reconcileSearch()} disabled={isRunningSearchOps() || !activeWorkspace()}>
                Reconcile Index
              </button>
            </div>
          </Show>
          <Show when={searchOpsStatus()}>
            <p class="status ok">{searchOpsStatus()}</p>
          </Show>
          <Show when={searchError()}>
            <p class="status error">{searchError()}</p>
          </Show>
          <Show when={searchResults()}>
            {(results) => (
              <ul>
                <For each={results().messages}>
                  {(message) => (
                    <li>
                      <span class="presence online" />
                      {displayUserLabel(message.authorId)}: {(tokenizeToDisplayText(message.markdownTokens) || message.content).slice(0, 40)}
                    </li>
                  )}
                </For>
              </ul>
            )}
          </Show>
          </section>
        </Show>

        <Show when={canAccessActiveChannel()}>
          <section class="member-group">
          <p class="group-label">ATTACHMENTS</p>
          <form class="inline-form" onSubmit={uploadAttachment}>
            <label>
              File
              <input
                type="file"
                onInput={(event) => {
                  const file = event.currentTarget.files?.[0] ?? null;
                  setSelectedAttachment(file);
                  setAttachmentFilename(file?.name ?? "");
                }}
              />
            </label>
            <label>
              Filename
              <input
                value={attachmentFilename()}
                onInput={(event) => setAttachmentFilename(event.currentTarget.value)}
                maxlength="128"
                placeholder="upload.bin"
              />
            </label>
            <button type="submit" disabled={isUploadingAttachment() || !activeChannel()}>
              {isUploadingAttachment() ? "Uploading..." : "Upload"}
            </button>
          </form>
          <Show when={attachmentStatus()}>
            <p class="status ok">{attachmentStatus()}</p>
          </Show>
          <Show when={attachmentError()}>
            <p class="status error">{attachmentError()}</p>
          </Show>
          <ul>
            <For each={activeAttachments()}>
              {(record) => (
                <li>
                  <span class="presence online" />
                  <div class="stacked-meta">
                    <span>{record.filename}</span>
                    <span class="muted mono">{record.mimeType} · {formatBytes(record.sizeBytes)}</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => void downloadAttachment(record)}
                    disabled={downloadingAttachmentId() === record.attachmentId}
                  >
                    {downloadingAttachmentId() === record.attachmentId ? "..." : "Get"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void removeAttachment(record)}
                    disabled={deletingAttachmentId() === record.attachmentId}
                  >
                    {deletingAttachmentId() === record.attachmentId ? "..." : "Del"}
                  </button>
                </li>
              )}
            </For>
            <Show when={activeAttachments().length === 0}>
              <li>
                <span class="presence idle" />
                no-local-attachments
              </li>
            </Show>
          </ul>
          </section>
        </Show>

        <Show when={canAccessActiveChannel()}>
          <section class="member-group">
          <p class="group-label">VOICE TOKEN</p>
          <form class="inline-form" onSubmit={requestVoiceToken}>
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={voiceCanPublish()}
                onChange={(event) => setVoiceCanPublish(event.currentTarget.checked)}
              />
              can_publish
            </label>
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={voiceCanSubscribe()}
                onChange={(event) => setVoiceCanSubscribe(event.currentTarget.checked)}
              />
              can_subscribe
            </label>
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={voiceMicrophone()}
                onChange={(event) => setVoiceMicrophone(event.currentTarget.checked)}
              />
              microphone
            </label>
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={voiceCamera()}
                onChange={(event) => setVoiceCamera(event.currentTarget.checked)}
              />
              camera
            </label>
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={voiceScreenShare()}
                onChange={(event) => setVoiceScreenShare(event.currentTarget.checked)}
              />
              screen_share
            </label>
            <button type="submit" disabled={isIssuingVoiceToken() || !activeChannel()}>
              {isIssuingVoiceToken() ? "Issuing..." : "Issue token"}
            </button>
          </form>
          <Show when={voiceTokenPreview()}>
            {(prefix) => <p class="mono">token_prefix: {prefix()}...</p>}
          </Show>
          <Show when={voiceTokenStatus()}>
            <p class="status ok">{voiceTokenStatus()}</p>
          </Show>
          <Show when={voiceTokenError()}>
            <p class="status error">{voiceTokenError()}</p>
          </Show>
          </section>
        </Show>

        <Show when={hasModerationAccess()}>
          <section class="member-group">
          <p class="group-label">MODERATION</p>
          <form class="inline-form">
            <label>
              Target user ULID
              <input
                value={moderationUserIdInput()}
                onInput={(event) => setModerationUserIdInput(event.currentTarget.value)}
                maxlength="26"
                placeholder="01ARZ..."
              />
            </label>
            <label>
              Role
              <select
                value={moderationRoleInput()}
                onChange={(event) => setModerationRoleInput(roleFromInput(event.currentTarget.value))}
              >
                <option value="member">member</option>
                <option value="moderator">moderator</option>
                <option value="owner">owner</option>
              </select>
            </label>
            <div class="button-row">
              <Show when={canManageRoles()}>
                <button
                  type="button"
                  disabled={isModerating() || !activeWorkspace()}
                  onClick={() => void runMemberAction("add")}
                >
                  Add
                </button>
                <button type="button" disabled={isModerating() || !activeWorkspace()} onClick={() => void runMemberAction("role")}>
                  Set Role
                </button>
              </Show>
              <Show when={canBanMembers()}>
                <button type="button" disabled={isModerating() || !activeWorkspace()} onClick={() => void runMemberAction("kick")}>
                  Kick
                </button>
                <button type="button" disabled={isModerating() || !activeWorkspace()} onClick={() => void runMemberAction("ban")}>
                  Ban
                </button>
              </Show>
            </div>
          </form>
          <Show when={canManageChannelOverrides()}>
            <form class="inline-form" onSubmit={applyOverride}>
              <label>
                Override role
                <select
                  value={overrideRoleInput()}
                  onChange={(event) => setOverrideRoleInput(roleFromInput(event.currentTarget.value))}
                >
                  <option value="member">member</option>
                  <option value="moderator">moderator</option>
                  <option value="owner">owner</option>
                </select>
              </label>
              <label>
                Allow permissions (csv)
                <input
                  value={overrideAllowCsv()}
                  onInput={(event) => setOverrideAllowCsv(event.currentTarget.value)}
                  placeholder="create_message,subscribe_streams"
                />
              </label>
              <label>
                Deny permissions (csv)
                <input
                  value={overrideDenyCsv()}
                  onInput={(event) => setOverrideDenyCsv(event.currentTarget.value)}
                  placeholder="delete_message"
                />
              </label>
              <button type="submit" disabled={isModerating() || !activeChannel()}>
                Apply channel override
              </button>
            </form>
          </Show>
          <Show when={moderationStatus()}>
            <p class="status ok">{moderationStatus()}</p>
          </Show>
          <Show when={moderationError()}>
            <p class="status error">{moderationError()}</p>
          </Show>
          </section>
        </Show>

        <section class="member-group">
          <p class="group-label">UTILITY</p>
          <div class="button-row">
            <button type="button" onClick={() => void runHealthCheck()} disabled={isCheckingHealth()}>
              {isCheckingHealth() ? "Checking..." : "Health"}
            </button>
          </div>
          <form class="inline-form" onSubmit={runEcho}>
            <label>
              Echo
              <input
                value={echoInput()}
                onInput={(event) => setEchoInput(event.currentTarget.value)}
                maxlength="128"
              />
            </label>
            <button type="submit" disabled={isEchoing()}>
              {isEchoing() ? "Sending..." : "Echo"}
            </button>
          </form>
          <Show when={healthStatus()}>
            <p class="status ok">{healthStatus()}</p>
          </Show>
          <Show when={diagError()}>
            <p class="status error">{diagError()}</p>
          </Show>
        </section>
      </aside>
    </div>
  );
}
