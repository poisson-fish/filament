import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AccessToken, AuthSession } from "../../../domain/auth";
import type {
  ChannelRecord,
  ChannelId,
  FriendRecord,
  FriendRequestList,
  GuildId,
  GuildRoleRecord,
  MessageId,
  MessageRecord,
  PermissionName,
  ReactionEmoji,
  RoleColorHex,
  UserId,
  WorkspaceRoleId,
  WorkspaceRoleName,
  WorkspaceRecord,
} from "../../../domain/chat";
import {
  userIdFromInput,
  workspaceRoleNameFromInput,
} from "../../../domain/chat";
import {
  connectGateway,
  type WorkspaceChannelOverrideUpdatePayload,
  type WorkspaceIpBanSyncPayload,
  type WorkspaceRoleAssignmentAddPayload,
  type WorkspaceRoleAssignmentRemovePayload,
  type WorkspaceRoleCreatePayload,
  type WorkspaceRoleDeletePayload,
  type WorkspaceRoleReorderPayload,
  type WorkspaceRoleUpdatePayload,
  type FriendRemovePayload,
  type FriendRequestCreatePayload,
  type FriendRequestDeletePayload,
  type FriendRequestUpdatePayload,
  type MessageDeletePayload,
  type MessageReactionPayload,
  type MessageUpdatePayload,
  type ProfileAvatarUpdatePayload,
  type ProfileBannerUpdatePayload,
  type ProfileUpdatePayload,
  type WorkspaceMemberBanPayload,
  type WorkspaceMemberRemovePayload,
  type WorkspaceUpdatePayload,
  type VoiceParticipantJoinPayload,
  type VoiceParticipantLeavePayload,
  type VoiceParticipantPayload,
  type VoiceParticipantSyncPayload,
  type VoiceParticipantUpdatePayload,
  type VoiceStreamPublishPayload,
  type VoiceStreamUnpublishPayload,
} from "../../../lib/gateway";
import {
  clearKeysByPrefix,
  mergeMessage,
  mergeReactionStateFromMessages,
  reactionKey,
  upsertWorkspace,
  upsertReactionEntry,
  type ReactionView,
} from "../helpers";
import { primeUsernameCache } from "../../../lib/username-cache";

const MAX_WORKSPACE_ROLE_PERMISSIONS = 64;

export interface GatewayControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  workspaces: Accessor<WorkspaceRecord[]>;
  canAccessActiveChannel: Accessor<boolean>;
  setGatewayOnline: Setter<boolean>;
  setOnlineMembers: Setter<string[]>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setMessages: Setter<MessageRecord[]>;
  setReactionState: Setter<Record<string, ReactionView>>;
  setResolvedUsernames: Setter<Record<string, string>>;
  setAvatarVersionByUserId: Setter<Record<string, number>>;
  setBannerVersionByUserId?: Setter<Record<string, number>>;
  setProfileDraftUsername: Setter<string>;
  setProfileDraftAbout: Setter<string>;
  setFriends: Setter<FriendRecord[]>;
  setFriendRequests: Setter<FriendRequestList>;
  setVoiceParticipantsByChannel: Setter<Record<string, VoiceParticipantPayload[]>>;
  isMessageListNearBottom: () => boolean;
  scrollMessageListToBottom: () => void;
  onGatewayConnectionChange?: (isOpen: boolean) => void;
  upsertWorkspaceRoleForGuild?: (
    guildId: GuildId,
    role: GuildRoleRecord,
  ) => void;
  updateWorkspaceRoleForGuild?: (
    guildId: GuildId,
    roleId: WorkspaceRoleId,
    updatedFields: {
      name?: WorkspaceRoleName;
      permissions?: ReadonlyArray<PermissionName>;
      colorHex?: RoleColorHex | null;
    },
  ) => void;
  removeWorkspaceRoleFromGuild?: (
    guildId: GuildId,
    roleId: WorkspaceRoleId,
  ) => void;
  reorderWorkspaceRolesForGuild?: (
    guildId: GuildId,
    orderedRoleIds: ReadonlyArray<WorkspaceRoleId>,
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
  setLegacyChannelOverride?: (
    guildId: GuildId,
    channelId: ChannelId,
    role: WorkspaceChannelOverrideUpdatePayload["role"],
    allow: ReadonlyArray<PermissionName>,
    deny: ReadonlyArray<PermissionName>,
    updatedAtUnix: number | null,
  ) => void;
  onWorkspacePermissionsChanged?: (guildId: GuildId) => void;
  onWorkspaceModerationChanged?: (payload: WorkspaceIpBanSyncPayload) => void;
}

interface GatewayClient {
  setSubscribedChannels: (guildId: GuildId, channelIds: ReadonlyArray<ChannelId>) => void;
  close: () => void;
}

interface GatewayHandlers {
  onReady: (payload: { userId: string }) => void;
  onOpenStateChange: (isOpen: boolean) => void;
  onMessageCreate: (message: MessageRecord) => void;
  onMessageUpdate: (payload: MessageUpdatePayload) => void;
  onMessageDelete: (payload: MessageDeletePayload) => void;
  onMessageReaction: (payload: MessageReactionPayload) => void;
  onChannelCreate: (payload: {
    guildId: GuildId;
    channel: ChannelRecord;
  }) => void;
  onWorkspaceUpdate: (payload: WorkspaceUpdatePayload) => void;
  onWorkspaceMemberAdd: (_payload: unknown) => void;
  onWorkspaceMemberUpdate: (_payload: unknown) => void;
  onWorkspaceMemberRemove: (payload: WorkspaceMemberRemovePayload) => void;
  onWorkspaceMemberBan: (payload: WorkspaceMemberBanPayload) => void;
  onWorkspaceRoleCreate: (payload: WorkspaceRoleCreatePayload) => void;
  onWorkspaceRoleUpdate: (payload: WorkspaceRoleUpdatePayload) => void;
  onWorkspaceRoleDelete: (payload: WorkspaceRoleDeletePayload) => void;
  onWorkspaceRoleReorder: (payload: WorkspaceRoleReorderPayload) => void;
  onWorkspaceRoleAssignmentAdd: (payload: WorkspaceRoleAssignmentAddPayload) => void;
  onWorkspaceRoleAssignmentRemove: (payload: WorkspaceRoleAssignmentRemovePayload) => void;
  onWorkspaceChannelOverrideUpdate: (payload: WorkspaceChannelOverrideUpdatePayload) => void;
  onWorkspaceIpBanSync: (payload: WorkspaceIpBanSyncPayload) => void;
  onProfileUpdate: (payload: ProfileUpdatePayload) => void;
  onProfileAvatarUpdate: (payload: ProfileAvatarUpdatePayload) => void;
  onProfileBannerUpdate: (_payload: ProfileBannerUpdatePayload) => void;
  onFriendRequestCreate: (payload: FriendRequestCreatePayload) => void;
  onFriendRequestUpdate: (payload: FriendRequestUpdatePayload) => void;
  onFriendRequestDelete: (payload: FriendRequestDeletePayload) => void;
  onFriendRemove: (payload: FriendRemovePayload) => void;
  onPresenceSync: (payload: { guildId: GuildId; userIds: string[] }) => void;
  onPresenceUpdate: (payload: {
    guildId: GuildId;
    userId: string;
    status: "online" | "offline";
  }) => void;
  onVoiceParticipantSync: (payload: VoiceParticipantSyncPayload) => void;
  onVoiceParticipantJoin: (payload: VoiceParticipantJoinPayload) => void;
  onVoiceParticipantLeave: (payload: VoiceParticipantLeavePayload) => void;
  onVoiceParticipantUpdate: (payload: VoiceParticipantUpdatePayload) => void;
  onVoiceStreamPublish: (payload: VoiceStreamPublishPayload) => void;
  onVoiceStreamUnpublish: (payload: VoiceStreamUnpublishPayload) => void;
}

export interface GatewayControllerDependencies {
  connectGateway: (
    accessToken: AccessToken,
    guildId: GuildId,
    channelId: ChannelId,
    handlers: GatewayHandlers,
  ) => GatewayClient;
}

export function applyMessageReactionUpdate(
  existing: Record<string, ReactionView>,
  payload: {
    messageId: MessageId;
    emoji: ReactionEmoji;
    count: number;
  },
): Record<string, ReactionView> {
  const key = reactionKey(payload.messageId, payload.emoji);
  const nextReacted =
    payload.count === 0 ? false : (existing[key]?.reacted ?? false);
  return upsertReactionEntry(existing, key, {
    count: payload.count,
    reacted: nextReacted,
  });
}

export function applyMessageUpdate(
  existing: MessageRecord[],
  payload: MessageUpdatePayload,
): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === payload.messageId);
  if (index < 0) {
    return existing;
  }
  const current = existing[index]!;
  const updated: MessageRecord = {
    ...current,
    content: payload.updatedFields.content ?? current.content,
    markdownTokens: payload.updatedFields.markdownTokens ?? current.markdownTokens,
  };
  return mergeMessage(existing, updated);
}

export function applyMessageDelete(
  existing: MessageRecord[],
  payload: MessageDeletePayload,
): MessageRecord[] {
  return existing.filter((entry) => entry.messageId !== payload.messageId);
}

const DEFAULT_GATEWAY_CONTROLLER_DEPENDENCIES: GatewayControllerDependencies = {
  connectGateway,
};

export function applyPresenceUpdate(
  existing: string[],
  payload: {
    userId: string;
    status: "online" | "offline";
  },
): string[] {
  if (payload.status === "online") {
    return existing.includes(payload.userId)
      ? existing
      : [...existing, payload.userId];
  }
  return existing.filter((entry) => entry !== payload.userId);
}

export function applyChannelCreate(
  existing: WorkspaceRecord[],
  payload: {
    guildId: GuildId;
    channel: ChannelRecord;
  },
): WorkspaceRecord[] {
  return upsertWorkspace(existing, payload.guildId, (workspace) => {
    if (
      workspace.channels.some(
        (channel) => channel.channelId === payload.channel.channelId,
      )
    ) {
      return workspace;
    }
    return {
      ...workspace,
      channels: [...workspace.channels, payload.channel],
    };
  });
}

export function applyWorkspaceUpdate(
  existing: WorkspaceRecord[],
  payload: WorkspaceUpdatePayload,
): WorkspaceRecord[] {
  return upsertWorkspace(existing, payload.guildId, (workspace) => ({
    ...workspace,
    guildName: payload.updatedFields.name ?? workspace.guildName,
    visibility: payload.updatedFields.visibility ?? workspace.visibility,
  }));
}

function normalizePermissions(
  permissions: ReadonlyArray<PermissionName>,
): PermissionName[] | null {
  if (permissions.length > MAX_WORKSPACE_ROLE_PERMISSIONS) {
    return null;
  }
  return [...new Set(permissions)];
}

function toWorkspaceRoleRecord(
  payload: WorkspaceRoleCreatePayload["role"],
): GuildRoleRecord | null {
  let name: WorkspaceRoleName;
  try {
    name = workspaceRoleNameFromInput(payload.name);
  } catch {
    return null;
  }
  const permissions = normalizePermissions(payload.permissions);
  if (!permissions) {
    return null;
  }
  const role: GuildRoleRecord = {
    roleId: payload.roleId,
    name,
    position: payload.position,
    isSystem: payload.isSystem,
    permissions,
  };
  if (typeof payload.colorHex !== "undefined") {
    role.colorHex = payload.colorHex;
  }

  return role;
}

function toWorkspaceRoleUpdatedFields(
  payload: WorkspaceRoleUpdatePayload,
): {
  name?: WorkspaceRoleName;
  permissions?: PermissionName[];
  colorHex?: RoleColorHex | null;
} | null {
  const updatedFields: {
    name?: WorkspaceRoleName;
    permissions?: PermissionName[];
    colorHex?: RoleColorHex | null;
  } = {};
  if (typeof payload.updatedFields.name !== "undefined") {
    try {
      updatedFields.name = workspaceRoleNameFromInput(payload.updatedFields.name);
    } catch {
      return null;
    }
  }
  if (typeof payload.updatedFields.permissions !== "undefined") {
    const permissions = normalizePermissions(payload.updatedFields.permissions);
    if (!permissions) {
      return null;
    }
    updatedFields.permissions = permissions;
  }
  if (typeof payload.updatedFields.colorHex !== "undefined") {
    updatedFields.colorHex = payload.updatedFields.colorHex;
  }
  if (
    typeof updatedFields.name === "undefined" &&
    typeof updatedFields.permissions === "undefined" &&
    typeof updatedFields.colorHex === "undefined"
  ) {
    return null;
  }
  return updatedFields;
}

function parseGatewayUserId(userId: string): UserId | null {
  try {
    return userIdFromInput(userId);
  } catch {
    return null;
  }
}

function gatewaySubscriptionChannelIds(
  workspaces: WorkspaceRecord[],
  guildId: GuildId,
  activeChannelId: ChannelId,
): ChannelId[] {
  const channels: ChannelId[] = [activeChannelId];
  const workspace = workspaces.find((entry) => entry.guildId === guildId);
  if (!workspace) {
    return channels;
  }

  for (const channel of workspace.channels) {
    if (channel.kind !== "voice") {
      continue;
    }
    if (channel.channelId === activeChannelId) {
      continue;
    }
    channels.push(channel.channelId);
  }

  return channels;
}

function voiceChannelKey(guildId: GuildId, channelId: ChannelId): string {
  return `${guildId}|${channelId}`;
}

function voiceParticipantMatches(
  entry: VoiceParticipantPayload,
  candidate: {
    userId: string;
    identity: string;
  },
): boolean {
  return entry.identity === candidate.identity || entry.userId === candidate.userId;
}

function findVoiceParticipantIndex(
  participants: VoiceParticipantPayload[],
  candidate: {
    userId: string;
    identity: string;
  },
): number {
  return participants.findIndex((entry) => voiceParticipantMatches(entry, candidate));
}

function mergeVoiceParticipants(
  existing: VoiceParticipantPayload[],
  nextParticipant: VoiceParticipantPayload,
): VoiceParticipantPayload[] {
  const index = findVoiceParticipantIndex(existing, nextParticipant);
  if (index < 0) {
    return [...existing, nextParticipant];
  }
  const current = existing[index]!;
  if (nextParticipant.updatedAtUnix < current.updatedAtUnix) {
    return existing;
  }
  const next = existing.slice();
  next[index] = nextParticipant;
  return next;
}

function applyVoiceParticipantSyncState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceParticipantSyncPayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const currentParticipants = existing[key] ?? [];
  let syncedParticipants: VoiceParticipantPayload[] = [];
  for (const participant of payload.participants) {
    syncedParticipants = mergeVoiceParticipants(syncedParticipants, participant);
  }
  for (const participant of currentParticipants) {
    const participantIncludedInSync = syncedParticipants.some((syncedEntry) =>
      voiceParticipantMatches(syncedEntry, participant),
    );
    if (participantIncludedInSync) {
      continue;
    }
    if (participant.updatedAtUnix > payload.syncedAtUnix) {
      syncedParticipants = mergeVoiceParticipants(syncedParticipants, participant);
    }
  }
  return {
    ...existing,
    [key]: syncedParticipants,
  };
}

function applyVoiceParticipantJoinState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceParticipantJoinPayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const participants = existing[key] ?? [];
  return {
    ...existing,
    [key]: mergeVoiceParticipants(participants, payload.participant),
  };
}

function applyVoiceParticipantLeaveState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceParticipantLeavePayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const participants = existing[key];
  if (!participants) {
    return existing;
  }
  const next = participants.filter((entry) => {
    if (!voiceParticipantMatches(entry, payload)) {
      return true;
    }
    return payload.leftAtUnix < entry.updatedAtUnix;
  });
  if (next.length === participants.length) {
    return existing;
  }
  return {
    ...existing,
    [key]: next,
  };
}

function applyVoiceParticipantUpdateState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceParticipantUpdatePayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const participants = existing[key];
  if (!participants) {
    return existing;
  }
  const index = findVoiceParticipantIndex(participants, payload);
  if (index < 0) {
    return existing;
  }
  const current = participants[index]!;
  if (payload.updatedAtUnix < current.updatedAtUnix) {
    return existing;
  }
  const nextParticipant: VoiceParticipantPayload = {
    ...current,
    userId: payload.userId,
    identity: payload.identity,
    updatedAtUnix: payload.updatedAtUnix,
    isMuted: payload.updatedFields.isMuted ?? current.isMuted,
    isDeafened: payload.updatedFields.isDeafened ?? current.isDeafened,
    isSpeaking: payload.updatedFields.isSpeaking ?? current.isSpeaking,
    isVideoEnabled: payload.updatedFields.isVideoEnabled ?? current.isVideoEnabled,
    isScreenShareEnabled:
      payload.updatedFields.isScreenShareEnabled ?? current.isScreenShareEnabled,
  };
  const next = participants.slice();
  next[index] = nextParticipant;
  return {
    ...existing,
    [key]: next,
  };
}

function applyVoiceStreamPublishedState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceStreamPublishPayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const participants = existing[key];
  if (!participants) {
    return existing;
  }
  const index = findVoiceParticipantIndex(participants, payload);
  if (index < 0) {
    return existing;
  }
  const current = participants[index]!;
  if (payload.publishedAtUnix < current.updatedAtUnix) {
    return existing;
  }
  const nextParticipant = {
    ...current,
    userId: payload.userId,
    identity: payload.identity,
    updatedAtUnix: Math.max(current.updatedAtUnix, payload.publishedAtUnix),
    isVideoEnabled:
      payload.stream === "camera" ? true : current.isVideoEnabled,
    isScreenShareEnabled:
      payload.stream === "screen_share" ? true : current.isScreenShareEnabled,
  };
  const next = participants.slice();
  next[index] = nextParticipant;
  return {
    ...existing,
    [key]: next,
  };
}

function applyVoiceStreamUnpublishedState(
  existing: Record<string, VoiceParticipantPayload[]>,
  payload: VoiceStreamUnpublishPayload,
): Record<string, VoiceParticipantPayload[]> {
  const key = voiceChannelKey(payload.guildId, payload.channelId);
  const participants = existing[key];
  if (!participants) {
    return existing;
  }
  const index = findVoiceParticipantIndex(participants, payload);
  if (index < 0) {
    return existing;
  }
  const current = participants[index]!;
  if (payload.unpublishedAtUnix < current.updatedAtUnix) {
    return existing;
  }
  const nextParticipant = {
    ...current,
    userId: payload.userId,
    identity: payload.identity,
    updatedAtUnix: Math.max(current.updatedAtUnix, payload.unpublishedAtUnix),
    isVideoEnabled:
      payload.stream === "camera" ? false : current.isVideoEnabled,
    isScreenShareEnabled:
      payload.stream === "screen_share" ? false : current.isScreenShareEnabled,
  };
  const next = participants.slice();
  next[index] = nextParticipant;
  return {
    ...existing,
    [key]: next,
  };
}

function removeWorkspace(existing: WorkspaceRecord[], guildId: GuildId): WorkspaceRecord[] {
  return existing.filter((workspace) => workspace.guildId !== guildId);
}

function mergeResolvedUsername(
  existing: Record<string, string>,
  userId: string,
  username: string,
): Record<string, string> {
  if (existing[userId] === username) {
    return existing;
  }
  return {
    ...existing,
    [userId]: username,
  };
}

function mergeAvatarVersion(
  existing: Record<string, number>,
  userId: string,
  avatarVersion: number,
): Record<string, number> {
  const current = existing[userId] ?? 0;
  const nextVersion = Math.max(current, avatarVersion);
  if (current === nextVersion) {
    return existing;
  }
  return {
    ...existing,
    [userId]: nextVersion,
  };
}

function upsertFriend(
  existing: FriendRecord[],
  friend: FriendRecord,
): FriendRecord[] {
  const index = existing.findIndex((entry) => entry.userId === friend.userId);
  if (index < 0) {
    return [friend, ...existing];
  }
  const current = existing[index]!;
  if (
    current.username === friend.username &&
    current.createdAtUnix === friend.createdAtUnix
  ) {
    return existing;
  }
  const next = existing.slice();
  next[index] = friend;
  return next;
}

function removeFriend(
  existing: FriendRecord[],
  friendUserId: string,
): FriendRecord[] {
  return existing.filter((entry) => entry.userId !== friendUserId);
}

function upsertFriendRequestList(
  existing: FriendRequestList,
  payload: FriendRequestCreatePayload,
  connectedUserId: string,
): FriendRequestList {
  const request = {
    requestId: payload.requestId as FriendRequestList["incoming"][number]["requestId"],
    senderUserId: payload.senderUserId as FriendRequestList["incoming"][number]["senderUserId"],
    senderUsername: payload.senderUsername,
    recipientUserId: payload.recipientUserId as FriendRequestList["incoming"][number]["recipientUserId"],
    recipientUsername: payload.recipientUsername,
    createdAtUnix: payload.createdAtUnix,
  };
  if (payload.recipientUserId === connectedUserId) {
    const incoming = existing.incoming.filter(
      (entry) => entry.requestId !== request.requestId,
    );
    return {
      ...existing,
      incoming: [request, ...incoming],
    };
  }
  if (payload.senderUserId === connectedUserId) {
    const outgoing = existing.outgoing.filter(
      (entry) => entry.requestId !== request.requestId,
    );
    return {
      ...existing,
      outgoing: [request, ...outgoing],
    };
  }
  return existing;
}

function removeFriendRequest(
  existing: FriendRequestList,
  requestId: string,
): FriendRequestList {
  return {
    incoming: existing.incoming.filter((entry) => entry.requestId !== requestId),
    outgoing: existing.outgoing.filter((entry) => entry.requestId !== requestId),
  };
}

export function createGatewayController(
  options: GatewayControllerOptions,
  dependencies: Partial<GatewayControllerDependencies> = {},
): void {
  const deps = {
    ...DEFAULT_GATEWAY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };
  let connectedUserId: string | null = null;

  createEffect(() => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || !options.canAccessActiveChannel()) {
      options.setGatewayOnline(false);
      options.setOnlineMembers([]);
      return;
    }

    const gateway = deps.connectGateway(session.accessToken, guildId, channelId, {
      onReady: (payload) => {
        connectedUserId = payload.userId;
      },
      onOpenStateChange: (isOpen) => {
        options.setGatewayOnline(isOpen);
        options.onGatewayConnectionChange?.(isOpen);
      },
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        const shouldStickToBottom = options.isMessageListNearBottom();
        options.setMessages((existing) => mergeMessage(existing, message));
        options.setReactionState((existing) =>
          mergeReactionStateFromMessages(existing, [message]),
        );
        if (shouldStickToBottom) {
          options.scrollMessageListToBottom();
        }
      },
      onMessageUpdate: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setMessages((existing) => applyMessageUpdate(existing, payload));
      },
      onMessageDelete: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setMessages((existing) => applyMessageDelete(existing, payload));
        options.setReactionState((existing) =>
          clearKeysByPrefix(existing, `${payload.messageId}|`),
        );
      },
      onMessageReaction: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setReactionState((existing) =>
          applyMessageReactionUpdate(existing, payload),
        );
      },
      onChannelCreate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setWorkspaces((existing) => applyChannelCreate(existing, payload));
      },
      onWorkspaceUpdate: (payload) => {
        options.setWorkspaces((existing) => applyWorkspaceUpdate(existing, payload));
      },
      onWorkspaceMemberAdd: () => { },
      onWorkspaceMemberUpdate: () => { },
      onWorkspaceMemberRemove: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setWorkspaces((existing) => removeWorkspace(existing, payload.guildId));
      },
      onWorkspaceMemberBan: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setWorkspaces((existing) => removeWorkspace(existing, payload.guildId));
      },
      onWorkspaceRoleCreate: (payload) => {
        const role = toWorkspaceRoleRecord(payload.role);
        if (role) {
          options.upsertWorkspaceRoleForGuild?.(payload.guildId, role);
        }
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleUpdate: (payload) => {
        const updatedFields = toWorkspaceRoleUpdatedFields(payload);
        if (updatedFields) {
          options.updateWorkspaceRoleForGuild?.(
            payload.guildId,
            payload.roleId,
            updatedFields,
          );
        }
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleDelete: (payload) => {
        options.removeWorkspaceRoleFromGuild?.(payload.guildId, payload.roleId);
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleReorder: (payload) => {
        options.reorderWorkspaceRolesForGuild?.(payload.guildId, payload.roleIds);
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleAssignmentAdd: (payload) => {
        const userId = parseGatewayUserId(payload.userId);
        if (userId) {
          options.assignWorkspaceRoleToUser?.(
            payload.guildId,
            userId,
            payload.roleId,
          );
        }
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleAssignmentRemove: (payload) => {
        const userId = parseGatewayUserId(payload.userId);
        if (userId) {
          options.unassignWorkspaceRoleFromUser?.(
            payload.guildId,
            userId,
            payload.roleId,
          );
        }
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceChannelOverrideUpdate: (payload) => {
        options.setLegacyChannelOverride?.(
          payload.guildId,
          payload.channelId,
          payload.role,
          payload.updatedFields.allow,
          payload.updatedFields.deny,
          payload.updatedAtUnix,
        );
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceIpBanSync: (payload) => {
        options.onWorkspaceModerationChanged?.(payload);
      },
      onProfileUpdate: (payload) => {
        if (payload.updatedFields.username) {
          primeUsernameCache([{
            userId: payload.userId as import("../../../domain/chat").UserId,
            username: payload.updatedFields.username
          }]);
        }
        options.setResolvedUsernames((existing) => {
          if (!payload.updatedFields.username) {
            return existing;
          }
          return mergeResolvedUsername(existing, payload.userId, payload.updatedFields.username);
        });
        if (payload.userId !== connectedUserId) {
          return;
        }
        if (payload.updatedFields.username) {
          options.setProfileDraftUsername(payload.updatedFields.username);
        }
        if (payload.updatedFields.aboutMarkdown) {
          options.setProfileDraftAbout(payload.updatedFields.aboutMarkdown);
        }
      },
      onProfileAvatarUpdate: (payload) => {
        primeUsernameCache([{
          userId: payload.userId as import("../../../domain/chat").UserId,
          avatarVersion: payload.avatarVersion
        }]);
        options.setAvatarVersionByUserId((existing) =>
          mergeAvatarVersion(existing, payload.userId, payload.avatarVersion),
        );
      },
      onProfileBannerUpdate: (payload) => {
        options.setBannerVersionByUserId?.((existing) =>
          mergeAvatarVersion(existing, payload.userId, payload.bannerVersion),
        );
      },
      onFriendRequestCreate: (payload) => {
        options.setResolvedUsernames((existing) => {
          let next = mergeResolvedUsername(
            existing,
            payload.senderUserId,
            payload.senderUsername,
          );
          next = mergeResolvedUsername(
            next,
            payload.recipientUserId,
            payload.recipientUsername,
          );
          return next;
        });
        if (!connectedUserId) {
          return;
        }
        const targetUserId = connectedUserId;
        options.setFriendRequests((existing) =>
          upsertFriendRequestList(existing, payload, targetUserId),
        );
      },
      onFriendRequestUpdate: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setResolvedUsernames((existing) =>
          mergeResolvedUsername(existing, payload.friendUserId, payload.friendUsername),
        );
        options.setFriendRequests((existing) =>
          removeFriendRequest(existing, payload.requestId),
        );
        options.setFriends((existing) =>
          upsertFriend(existing, {
            userId: payload.friendUserId as FriendRecord["userId"],
            username: payload.friendUsername,
            createdAtUnix: payload.friendshipCreatedAtUnix,
          }),
        );
      },
      onFriendRequestDelete: (payload) => {
        options.setFriendRequests((existing) =>
          removeFriendRequest(existing, payload.requestId),
        );
      },
      onFriendRemove: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setFriends((existing) => removeFriend(existing, payload.friendUserId));
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setOnlineMembers((existing) =>
          applyPresenceUpdate(existing, payload),
        );
      },
      onVoiceParticipantSync: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceParticipantSyncState(existing, payload),
        );
      },
      onVoiceParticipantJoin: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceParticipantJoinState(existing, payload),
        );
      },
      onVoiceParticipantLeave: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceParticipantLeaveState(existing, payload),
        );
      },
      onVoiceParticipantUpdate: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceParticipantUpdateState(existing, payload),
        );
      },
      onVoiceStreamPublish: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceStreamPublishedState(existing, payload),
        );
      },
      onVoiceStreamUnpublish: (payload) => {
        options.setVoiceParticipantsByChannel((existing) =>
          applyVoiceStreamUnpublishedState(existing, payload),
        );
      },
    });

    createEffect(() => {
      gateway.setSubscribedChannels(
        guildId,
        gatewaySubscriptionChannelIds(
          options.workspaces(),
          guildId,
          channelId,
        ),
      );
    });

    onCleanup(() => gateway.close());
  });
}
