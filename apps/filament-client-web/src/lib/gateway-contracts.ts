import {
  type ChannelId,
  type ChannelRecord,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type MarkdownToken,
  type MessageId,
  type MessageRecord,
  type PermissionName,
  type ReactionEmoji,
  type RoleName,
  type WorkspaceRoleId,
} from "../domain/chat";
import {
  type PresenceSyncPayload,
  type PresenceUpdatePayload,
} from "./gateway-presence-events";
import {
  type ReadyPayload,
} from "./gateway-ready-dispatch";

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

export interface ChannelCreatePayload {
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

export interface GatewayHandlers {
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