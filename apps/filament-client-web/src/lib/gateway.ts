import { type AccessToken } from "../domain/auth";
import {
  parseGatewayEventEnvelope,
} from "./gateway-envelope";
import {
  decodeFriendGatewayEvent,
} from "./gateway-friend-events";
import {
  decodePresenceGatewayEvent,
  type PresenceSyncPayload,
  type PresenceUpdatePayload,
} from "./gateway-presence-events";
import {
  decodeProfileGatewayEvent,
} from "./gateway-profile-events";
import {
  decodeVoiceGatewayEvent,
} from "./gateway-voice-events";
import {
  channelFromResponse,
  channelIdFromInput,
  type ChannelRecord,
  type ChannelId,
  type GuildId,
  guildNameFromInput,
  type GuildName,
  type GuildVisibility,
  guildVisibilityFromInput,
  markdownTokensFromResponse,
  messageContentFromInput,
  type MessageId,
  type MarkdownToken,
  type MessageRecord,
  type ReactionEmoji,
  roleFromInput,
  permissionFromInput,
  type RoleName,
  type PermissionName,
  type WorkspaceRoleId,
  guildIdFromInput,
  messageIdFromInput,
  messageFromResponse,
  reactionEmojiFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
} from "../domain/chat";

const MAX_WORKSPACE_ROLE_REORDER_IDS = 64;

interface ReadyPayload {
  userId: string;
}

export type VoiceStreamKind = "microphone" | "camera" | "screen_share";

export interface VoiceParticipantPayload {
  userId: string;
  identity: string;
  joinedAtUnix: number;
  updatedAtUnix: number;
  isMuted: boolean;
  isDeafened: boolean;
  isSpeaking: boolean;
  isVideoEnabled: boolean;
  isScreenShareEnabled: boolean;
}

export interface VoiceParticipantSyncPayload {
  guildId: GuildId;
  channelId: ChannelId;
  participants: VoiceParticipantPayload[];
  syncedAtUnix: number;
}

export interface VoiceParticipantJoinPayload {
  guildId: GuildId;
  channelId: ChannelId;
  participant: VoiceParticipantPayload;
}

export interface VoiceParticipantLeavePayload {
  guildId: GuildId;
  channelId: ChannelId;
  userId: string;
  identity: string;
  leftAtUnix: number;
}

export interface VoiceParticipantUpdatePayload {
  guildId: GuildId;
  channelId: ChannelId;
  userId: string;
  identity: string;
  updatedFields: {
    isMuted?: boolean;
    isDeafened?: boolean;
    isSpeaking?: boolean;
    isVideoEnabled?: boolean;
    isScreenShareEnabled?: boolean;
  };
  updatedAtUnix: number;
}

export interface VoiceStreamPublishPayload {
  guildId: GuildId;
  channelId: ChannelId;
  userId: string;
  identity: string;
  stream: VoiceStreamKind;
  publishedAtUnix: number;
}

export interface VoiceStreamUnpublishPayload {
  guildId: GuildId;
  channelId: ChannelId;
  userId: string;
  identity: string;
  stream: VoiceStreamKind;
  unpublishedAtUnix: number;
}

interface ChannelCreatePayload {
  guildId: GuildId;
  channel: ChannelRecord;
}

export interface WorkspaceUpdatePayload {
  guildId: GuildId;
  updatedFields: {
    name?: GuildName;
    visibility?: GuildVisibility;
  };
  updatedAtUnix: number;
}

export interface WorkspaceMemberAddPayload {
  guildId: GuildId;
  userId: string;
  role: RoleName;
  joinedAtUnix: number;
}

export interface WorkspaceMemberUpdatePayload {
  guildId: GuildId;
  userId: string;
  updatedFields: {
    role?: RoleName;
  };
  updatedAtUnix: number;
}

export type WorkspaceMemberRemoveReason = "kick" | "ban" | "leave";

export interface WorkspaceMemberRemovePayload {
  guildId: GuildId;
  userId: string;
  reason: WorkspaceMemberRemoveReason;
  removedAtUnix: number;
}

export interface WorkspaceMemberBanPayload {
  guildId: GuildId;
  userId: string;
  bannedAtUnix: number;
}

export interface WorkspaceRoleRecordPayload {
  roleId: WorkspaceRoleId;
  name: string;
  position: number;
  isSystem: boolean;
  permissions: PermissionName[];
}

export interface WorkspaceRoleCreatePayload {
  guildId: GuildId;
  role: WorkspaceRoleRecordPayload;
}

export interface WorkspaceRoleUpdatePayload {
  guildId: GuildId;
  roleId: WorkspaceRoleId;
  updatedFields: {
    name?: string;
    permissions?: PermissionName[];
  };
  updatedAtUnix: number;
}

export interface WorkspaceRoleDeletePayload {
  guildId: GuildId;
  roleId: WorkspaceRoleId;
  deletedAtUnix: number;
}

export interface WorkspaceRoleReorderPayload {
  guildId: GuildId;
  roleIds: WorkspaceRoleId[];
  updatedAtUnix: number;
}

export interface WorkspaceRoleAssignmentAddPayload {
  guildId: GuildId;
  userId: string;
  roleId: WorkspaceRoleId;
  assignedAtUnix: number;
}

export interface WorkspaceRoleAssignmentRemovePayload {
  guildId: GuildId;
  userId: string;
  roleId: WorkspaceRoleId;
  removedAtUnix: number;
}

export interface WorkspaceChannelOverrideUpdatePayload {
  guildId: GuildId;
  channelId: ChannelId;
  role: RoleName;
  updatedFields: {
    allow: PermissionName[];
    deny: PermissionName[];
  };
  updatedAtUnix: number;
}

export interface WorkspaceIpBanSyncPayload {
  guildId: GuildId;
  summary: {
    action: "upsert" | "remove";
    changedCount: number;
  };
  updatedAtUnix: number;
}

export interface ProfileUpdatePayload {
  userId: string;
  updatedFields: {
    username?: string;
    aboutMarkdown?: string;
    aboutMarkdownTokens?: MarkdownToken[];
  };
  updatedAtUnix: number;
}

export interface ProfileAvatarUpdatePayload {
  userId: string;
  avatarVersion: number;
  updatedAtUnix: number;
}

export interface FriendRequestCreatePayload {
  requestId: string;
  senderUserId: string;
  senderUsername: string;
  recipientUserId: string;
  recipientUsername: string;
  createdAtUnix: number;
}

export interface FriendRequestUpdatePayload {
  requestId: string;
  state: "accepted";
  userId: string;
  friendUserId: string;
  friendUsername: string;
  friendshipCreatedAtUnix: number;
  updatedAtUnix: number;
}

export interface FriendRequestDeletePayload {
  requestId: string;
  deletedAtUnix: number;
}

export interface FriendRemovePayload {
  userId: string;
  friendUserId: string;
  removedAtUnix: number;
}

export interface MessageReactionPayload {
  guildId: GuildId;
  channelId: ChannelId;
  messageId: MessageId;
  emoji: ReactionEmoji;
  count: number;
}

export interface MessageUpdatePayload {
  guildId: GuildId;
  channelId: ChannelId;
  messageId: MessageId;
  updatedFields: {
    content?: MessageRecord["content"];
    markdownTokens?: MarkdownToken[];
  };
  updatedAtUnix: number;
}

export interface MessageDeletePayload {
  guildId: GuildId;
  channelId: ChannelId;
  messageId: MessageId;
  deletedAtUnix: number;
}

interface GatewayHandlers {
  onReady?: (payload: ReadyPayload) => void;
  onMessageCreate?: (message: MessageRecord) => void;
  onMessageUpdate?: (payload: MessageUpdatePayload) => void;
  onMessageDelete?: (payload: MessageDeletePayload) => void;
  onMessageReaction?: (payload: MessageReactionPayload) => void;
  onChannelCreate?: (payload: ChannelCreatePayload) => void;
  onWorkspaceUpdate?: (payload: WorkspaceUpdatePayload) => void;
  onWorkspaceMemberAdd?: (payload: WorkspaceMemberAddPayload) => void;
  onWorkspaceMemberUpdate?: (payload: WorkspaceMemberUpdatePayload) => void;
  onWorkspaceMemberRemove?: (payload: WorkspaceMemberRemovePayload) => void;
  onWorkspaceMemberBan?: (payload: WorkspaceMemberBanPayload) => void;
  onWorkspaceRoleCreate?: (payload: WorkspaceRoleCreatePayload) => void;
  onWorkspaceRoleUpdate?: (payload: WorkspaceRoleUpdatePayload) => void;
  onWorkspaceRoleDelete?: (payload: WorkspaceRoleDeletePayload) => void;
  onWorkspaceRoleReorder?: (payload: WorkspaceRoleReorderPayload) => void;
  onWorkspaceRoleAssignmentAdd?: (payload: WorkspaceRoleAssignmentAddPayload) => void;
  onWorkspaceRoleAssignmentRemove?: (
    payload: WorkspaceRoleAssignmentRemovePayload,
  ) => void;
  onWorkspaceChannelOverrideUpdate?: (
    payload: WorkspaceChannelOverrideUpdatePayload,
  ) => void;
  onWorkspaceIpBanSync?: (payload: WorkspaceIpBanSyncPayload) => void;
  onProfileUpdate?: (payload: ProfileUpdatePayload) => void;
  onProfileAvatarUpdate?: (payload: ProfileAvatarUpdatePayload) => void;
  onFriendRequestCreate?: (payload: FriendRequestCreatePayload) => void;
  onFriendRequestUpdate?: (payload: FriendRequestUpdatePayload) => void;
  onFriendRequestDelete?: (payload: FriendRequestDeletePayload) => void;
  onFriendRemove?: (payload: FriendRemovePayload) => void;
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
  onVoiceParticipantSync?: (payload: VoiceParticipantSyncPayload) => void;
  onVoiceParticipantJoin?: (payload: VoiceParticipantJoinPayload) => void;
  onVoiceParticipantLeave?: (payload: VoiceParticipantLeavePayload) => void;
  onVoiceParticipantUpdate?: (payload: VoiceParticipantUpdatePayload) => void;
  onVoiceStreamPublish?: (payload: VoiceStreamPublishPayload) => void;
  onVoiceStreamUnpublish?: (payload: VoiceStreamUnpublishPayload) => void;
  onOpenStateChange?: (isOpen: boolean) => void;
}

interface GatewayClient {
  updateSubscription: (guildId: GuildId, channelId: ChannelId) => void;
  close: () => void;
}

function parseReadyPayload(payload: unknown): ReadyPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.user_id !== "string") {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return { userId };
}

function parseMessageReactionPayload(payload: unknown): MessageReactionPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.message_id !== "string" ||
    typeof value.emoji !== "string" ||
    typeof value.count !== "number" ||
    !Number.isSafeInteger(value.count) ||
    value.count < 0
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let messageId: MessageId;
  let emoji: ReactionEmoji;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    messageId = messageIdFromInput(value.message_id);
    emoji = reactionEmojiFromInput(value.emoji);
  } catch {
    return null;
  }

  return {
    guildId,
    channelId,
    messageId,
    emoji,
    count: value.count,
  };
}

function parseMessageUpdatePayload(payload: unknown): MessageUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.message_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let messageId: MessageId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    messageId = messageIdFromInput(value.message_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let content: MessageRecord["content"] | undefined;
  let markdownTokens: MarkdownToken[] | undefined;
  if (typeof updatedFieldsDto.content !== "undefined") {
    if (typeof updatedFieldsDto.content !== "string") {
      return null;
    }
    try {
      content = messageContentFromInput(updatedFieldsDto.content);
    } catch {
      return null;
    }
  }
  if (typeof updatedFieldsDto.markdown_tokens !== "undefined") {
    try {
      markdownTokens = markdownTokensFromResponse(updatedFieldsDto.markdown_tokens);
    } catch {
      return null;
    }
  }
  if (typeof content === "undefined" && typeof markdownTokens === "undefined") {
    return null;
  }

  return {
    guildId,
    channelId,
    messageId,
    updatedFields: {
      content,
      markdownTokens,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseMessageDeletePayload(payload: unknown): MessageDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.message_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let messageId: MessageId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    messageId = messageIdFromInput(value.message_id);
  } catch {
    return null;
  }

  return {
    guildId,
    channelId,
    messageId,
    deletedAtUnix: value.deleted_at_unix,
  };
}

function parseChannelCreatePayload(payload: unknown): ChannelCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string") {
    return null;
  }

  let guildId: GuildId;
  let channel: ChannelRecord;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channel = channelFromResponse(value.channel);
  } catch {
    return null;
  }

  return {
    guildId,
    channel,
  };
}

function parseWorkspaceUpdatePayload(payload: unknown): WorkspaceUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let name: GuildName | undefined;
  let visibility: GuildVisibility | undefined;
  if (typeof updatedFieldsDto.name !== "undefined") {
    if (typeof updatedFieldsDto.name !== "string") {
      return null;
    }
    try {
      name = guildNameFromInput(updatedFieldsDto.name);
    } catch {
      return null;
    }
  }
  if (typeof updatedFieldsDto.visibility !== "undefined") {
    if (typeof updatedFieldsDto.visibility !== "string") {
      return null;
    }
    try {
      visibility = guildVisibilityFromInput(updatedFieldsDto.visibility);
    } catch {
      return null;
    }
  }
  if (typeof name === "undefined" && typeof visibility === "undefined") {
    return null;
  }

  return {
    guildId,
    updatedFields: {
      name,
      visibility,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceMemberAddPayload(payload: unknown): WorkspaceMemberAddPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role !== "string" ||
    typeof value.joined_at_unix !== "number" ||
    !Number.isSafeInteger(value.joined_at_unix) ||
    value.joined_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let role: RoleName;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    role = roleFromInput(value.role);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    role,
    joinedAtUnix: value.joined_at_unix,
  };
}

function parseWorkspaceMemberUpdatePayload(payload: unknown): WorkspaceMemberUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let role: RoleName | undefined;
  if (typeof updatedFieldsDto.role !== "undefined") {
    if (typeof updatedFieldsDto.role !== "string") {
      return null;
    }
    try {
      role = roleFromInput(updatedFieldsDto.role);
    } catch {
      return null;
    }
  }
  if (typeof role === "undefined") {
    return null;
  }

  return {
    guildId,
    userId,
    updatedFields: { role },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceMemberRemovePayload(payload: unknown): WorkspaceMemberRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    (value.reason !== "kick" && value.reason !== "ban" && value.reason !== "leave") ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    reason: value.reason,
    removedAtUnix: value.removed_at_unix,
  };
}

function parseWorkspaceMemberBanPayload(payload: unknown): WorkspaceMemberBanPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.banned_at_unix !== "number" ||
    !Number.isSafeInteger(value.banned_at_unix) ||
    value.banned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    bannedAtUnix: value.banned_at_unix,
  };
}

function parseWorkspaceRolePayload(payload: unknown): WorkspaceRoleRecordPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.role_id !== "string" ||
    typeof value.name !== "string" ||
    typeof value.position !== "number" ||
    !Number.isSafeInteger(value.position) ||
    value.position < 1 ||
    typeof value.is_system !== "boolean" ||
    !Array.isArray(value.permissions)
  ) {
    return null;
  }

  let roleId: WorkspaceRoleId;
  try {
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  const permissions: PermissionName[] = [];
  for (const entry of value.permissions) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      permissions.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    roleId,
    name: value.name,
    position: value.position,
    isSystem: value.is_system,
    permissions,
  };
}

function parseWorkspaceRoleCreatePayload(payload: unknown): WorkspaceRoleCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string") {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }
  const role = parseWorkspaceRolePayload(value.role);
  if (!role) {
    return null;
  }

  return { guildId, role };
}

function parseWorkspaceRoleUpdatePayload(payload: unknown): WorkspaceRoleUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let name: string | undefined;
  let permissions: PermissionName[] | undefined;
  if (typeof updatedFieldsDto.name !== "undefined") {
    if (typeof updatedFieldsDto.name !== "string") {
      return null;
    }
    name = updatedFieldsDto.name;
  }
  if (typeof updatedFieldsDto.permissions !== "undefined") {
    if (!Array.isArray(updatedFieldsDto.permissions)) {
      return null;
    }
    permissions = [];
    for (const entry of updatedFieldsDto.permissions) {
      if (typeof entry !== "string") {
        return null;
      }
      try {
        permissions.push(permissionFromInput(entry));
      } catch {
        return null;
      }
    }
  }
  if (typeof name === "undefined" && typeof permissions === "undefined") {
    return null;
  }

  return {
    guildId,
    roleId,
    updatedFields: {
      name,
      permissions,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceRoleDeletePayload(payload: unknown): WorkspaceRoleDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    roleId,
    deletedAtUnix: value.deleted_at_unix,
  };
}

function parseWorkspaceRoleReorderPayload(payload: unknown): WorkspaceRoleReorderPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !Array.isArray(value.role_ids) ||
    value.role_ids.length > MAX_WORKSPACE_ROLE_REORDER_IDS ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const roleIds: WorkspaceRoleId[] = [];
  for (const entry of value.role_ids) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      roleIds.push(workspaceRoleIdFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    guildId,
    roleIds,
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceRoleAssignmentAddPayload(
  payload: unknown,
): WorkspaceRoleAssignmentAddPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.assigned_at_unix !== "number" ||
    !Number.isSafeInteger(value.assigned_at_unix) ||
    value.assigned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    roleId,
    assignedAtUnix: value.assigned_at_unix,
  };
}

function parseWorkspaceRoleAssignmentRemovePayload(
  payload: unknown,
): WorkspaceRoleAssignmentRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    roleId,
    removedAtUnix: value.removed_at_unix,
  };
}

function parseWorkspaceChannelOverrideUpdatePayload(
  payload: unknown,
): WorkspaceChannelOverrideUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.role !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let role: RoleName;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    role = roleFromInput(value.role);
  } catch {
    return null;
  }

  const updatedFields = value.updated_fields as Record<string, unknown>;
  if (!Array.isArray(updatedFields.allow) || !Array.isArray(updatedFields.deny)) {
    return null;
  }
  const allow: PermissionName[] = [];
  for (const entry of updatedFields.allow) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      allow.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }
  const deny: PermissionName[] = [];
  for (const entry of updatedFields.deny) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      deny.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    guildId,
    channelId,
    role,
    updatedFields: { allow, deny },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceIpBanSyncPayload(payload: unknown): WorkspaceIpBanSyncPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !value.summary ||
    typeof value.summary !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }
  const summaryDto = value.summary as Record<string, unknown>;
  if (
    (summaryDto.action !== "upsert" && summaryDto.action !== "remove") ||
    typeof summaryDto.changed_count !== "number" ||
    !Number.isSafeInteger(summaryDto.changed_count) ||
    summaryDto.changed_count < 0
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  return {
    guildId,
    summary: {
      action: summaryDto.action,
      changedCount: summaryDto.changed_count,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function normalizeGatewayBaseUrl(): string {
  const envGateway = import.meta.env.VITE_FILAMENT_GATEWAY_WS_URL;
  if (typeof envGateway === "string" && envGateway.length > 0) {
    return envGateway;
  }

  const envApi = import.meta.env.VITE_FILAMENT_API_BASE_URL;
  if (typeof envApi === "string" && /^https?:\/\//.test(envApi)) {
    const normalized = envApi.replace(/\/$/, "").replace(/\/api$/, "");
    return normalized.replace(/^http:\/\//, "ws://").replace(/^https:\/\//, "wss://");
  }

  return "";
}

export function resolveGatewayUrl(accessToken: AccessToken): string {
  const base = normalizeGatewayBaseUrl();
  const query = `access_token=${encodeURIComponent(accessToken)}`;
  if (base.length > 0) {
    return `${base}/gateway/ws?${query}`;
  }
  return `/gateway/ws?${query}`;
}

function sendEnvelope(socket: WebSocket, type: string, data: unknown): void {
  if (socket.readyState !== WebSocket.OPEN) {
    return;
  }
  socket.send(JSON.stringify({ v: 1, t: type, d: data }));
}

export function connectGateway(
  accessToken: AccessToken,
  guildId: GuildId,
  channelId: ChannelId,
  handlers: GatewayHandlers,
): GatewayClient {
  if (typeof WebSocket === "undefined") {
    handlers.onOpenStateChange?.(false);
    return {
      updateSubscription: () => {},
      close: () => {},
    };
  }

  let socket: WebSocket | null = null;
  let currentGuildId = guildId;
  let currentChannelId = channelId;
  let isClosed = false;
  let retryDelay = 1000;
  let reconnectTimer: number | null = null;

  const handleMessage = (event: MessageEvent) => {
    if (typeof event.data !== "string") {
      return;
    }
    const envelope = parseGatewayEventEnvelope(event.data);
    if (!envelope) {
      return;
    }

    if (envelope.t === "ready") {
      const payload = parseReadyPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onReady?.(payload);
      return;
    }

    if (envelope.t === "message_create") {
      try {
        handlers.onMessageCreate?.(messageFromResponse(envelope.d));
      } catch {
        return;
      }
      return;
    }

    if (envelope.t === "message_update") {
      const payload = parseMessageUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onMessageUpdate?.(payload);
      return;
    }

    if (envelope.t === "message_delete") {
      const payload = parseMessageDeletePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onMessageDelete?.(payload);
      return;
    }

    if (envelope.t === "message_reaction") {
      const payload = parseMessageReactionPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onMessageReaction?.(payload);
      return;
    }

    if (envelope.t === "channel_create") {
      const payload = parseChannelCreatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onChannelCreate?.(payload);
      return;
    }

    if (envelope.t === "workspace_update") {
      const payload = parseWorkspaceUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceUpdate?.(payload);
      return;
    }

    if (envelope.t === "workspace_member_add") {
      const payload = parseWorkspaceMemberAddPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceMemberAdd?.(payload);
      return;
    }

    if (envelope.t === "workspace_member_update") {
      const payload = parseWorkspaceMemberUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceMemberUpdate?.(payload);
      return;
    }

    if (envelope.t === "workspace_member_remove") {
      const payload = parseWorkspaceMemberRemovePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceMemberRemove?.(payload);
      return;
    }

    if (envelope.t === "workspace_member_ban") {
      const payload = parseWorkspaceMemberBanPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceMemberBan?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_create") {
      const payload = parseWorkspaceRoleCreatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleCreate?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_update") {
      const payload = parseWorkspaceRoleUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleUpdate?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_delete") {
      const payload = parseWorkspaceRoleDeletePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleDelete?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_reorder") {
      const payload = parseWorkspaceRoleReorderPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleReorder?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_assignment_add") {
      const payload = parseWorkspaceRoleAssignmentAddPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleAssignmentAdd?.(payload);
      return;
    }

    if (envelope.t === "workspace_role_assignment_remove") {
      const payload = parseWorkspaceRoleAssignmentRemovePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceRoleAssignmentRemove?.(payload);
      return;
    }

    if (envelope.t === "workspace_channel_override_update") {
      const payload = parseWorkspaceChannelOverrideUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceChannelOverrideUpdate?.(payload);
      return;
    }

    if (envelope.t === "workspace_ip_ban_sync") {
      const payload = parseWorkspaceIpBanSyncPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onWorkspaceIpBanSync?.(payload);
      return;
    }

    if (envelope.t === "profile_update" || envelope.t === "profile_avatar_update") {
      const profileEvent = decodeProfileGatewayEvent(envelope.t, envelope.d);
      if (!profileEvent) {
        return;
      }
      if (profileEvent.type === "profile_update") {
        handlers.onProfileUpdate?.(profileEvent.payload);
        return;
      }
      handlers.onProfileAvatarUpdate?.(profileEvent.payload);
      return;
    }

    if (
      envelope.t === "friend_request_create" ||
      envelope.t === "friend_request_update" ||
      envelope.t === "friend_request_delete" ||
      envelope.t === "friend_remove"
    ) {
      const friendEvent = decodeFriendGatewayEvent(envelope.t, envelope.d);
      if (!friendEvent) {
        return;
      }
      if (friendEvent.type === "friend_request_create") {
        handlers.onFriendRequestCreate?.(friendEvent.payload);
        return;
      }
      if (friendEvent.type === "friend_request_update") {
        handlers.onFriendRequestUpdate?.(friendEvent.payload);
        return;
      }
      if (friendEvent.type === "friend_request_delete") {
        handlers.onFriendRequestDelete?.(friendEvent.payload);
        return;
      }
      handlers.onFriendRemove?.(friendEvent.payload);
      return;
    }

    if (
      envelope.t === "voice_participant_sync" ||
      envelope.t === "voice_participant_join" ||
      envelope.t === "voice_participant_leave" ||
      envelope.t === "voice_participant_update" ||
      envelope.t === "voice_stream_publish" ||
      envelope.t === "voice_stream_unpublish"
    ) {
      const voiceEvent = decodeVoiceGatewayEvent(envelope.t, envelope.d);
      if (!voiceEvent) {
        return;
      }
      if (voiceEvent.type === "voice_participant_sync") {
        handlers.onVoiceParticipantSync?.(voiceEvent.payload);
        return;
      }
      if (voiceEvent.type === "voice_participant_join") {
        handlers.onVoiceParticipantJoin?.(voiceEvent.payload);
        return;
      }
      if (voiceEvent.type === "voice_participant_leave") {
        handlers.onVoiceParticipantLeave?.(voiceEvent.payload);
        return;
      }
      if (voiceEvent.type === "voice_participant_update") {
        handlers.onVoiceParticipantUpdate?.(voiceEvent.payload);
        return;
      }
      if (voiceEvent.type === "voice_stream_publish") {
        handlers.onVoiceStreamPublish?.(voiceEvent.payload);
        return;
      }
      handlers.onVoiceStreamUnpublish?.(voiceEvent.payload);
      return;
    }

    if (envelope.t === "presence_sync" || envelope.t === "presence_update") {
      const presenceEvent = decodePresenceGatewayEvent(envelope.t, envelope.d);
      if (!presenceEvent) {
        return;
      }
      if (presenceEvent.type === "presence_sync") {
        handlers.onPresenceSync?.(presenceEvent.payload);
      } else {
        handlers.onPresenceUpdate?.(presenceEvent.payload);
      }
    }
  };

  const connect = () => {
    if (isClosed) return;

    socket = new WebSocket(resolveGatewayUrl(accessToken));

    socket.onopen = () => {
      retryDelay = 1000;
      handlers.onOpenStateChange?.(true);
      if (socket && socket.readyState === WebSocket.OPEN) {
        sendEnvelope(socket, "subscribe", {
          guild_id: currentGuildId,
          channel_id: currentChannelId,
        });
      }
    };

    socket.onclose = () => {
      handlers.onOpenStateChange?.(false);
      if (!isClosed) {
        retryDelay = Math.min(retryDelay * 2, 30000);
        reconnectTimer = window.setTimeout(connect, retryDelay);
      }
    };

    socket.onmessage = handleMessage;
  };

  connect();

  return {
    updateSubscription: (nextGuildId, nextChannelId) => {
      currentGuildId = nextGuildId;
      currentChannelId = nextChannelId;
      if (socket && socket.readyState === WebSocket.OPEN) {
        sendEnvelope(socket, "subscribe", {
          guild_id: currentGuildId,
          channel_id: currentChannelId,
        });
      }
    },
    close: () => {
      isClosed = true;
      if (reconnectTimer) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (socket) {
        socket.onclose = null;
        if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) {
          socket.close();
        }
        socket = null;
      }
    },
  };
}
