import { type AccessToken } from "../domain/auth";
import {
  parseGatewayEventEnvelope,
} from "./gateway-envelope";
import {
  decodeFriendGatewayEvent,
} from "./gateway-friend-events";
import {
  decodeMessageGatewayEvent,
} from "./gateway-message-events";
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
  decodeWorkspaceGatewayEvent,
} from "./gateway-workspace-events";
import {
  type ChannelRecord,
  type ChannelId,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type MessageId,
  type MarkdownToken,
  type MessageRecord,
  type ReactionEmoji,
  type RoleName,
  type PermissionName,
  type WorkspaceRoleId,
  userIdFromInput,
} from "../domain/chat";

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

    if (
      envelope.t === "message_create" ||
      envelope.t === "message_update" ||
      envelope.t === "message_delete" ||
      envelope.t === "message_reaction"
    ) {
      const messageEvent = decodeMessageGatewayEvent(envelope.t, envelope.d);
      if (!messageEvent) {
        return;
      }
      if (messageEvent.type === "message_create") {
        handlers.onMessageCreate?.(messageEvent.payload);
        return;
      }
      if (messageEvent.type === "message_update") {
        handlers.onMessageUpdate?.(messageEvent.payload);
        return;
      }
      if (messageEvent.type === "message_delete") {
        handlers.onMessageDelete?.(messageEvent.payload);
        return;
      }
      handlers.onMessageReaction?.(messageEvent.payload);
      return;
    }

    if (
      envelope.t === "channel_create" ||
      envelope.t === "workspace_update" ||
      envelope.t === "workspace_member_add" ||
      envelope.t === "workspace_member_update" ||
      envelope.t === "workspace_member_remove" ||
      envelope.t === "workspace_member_ban" ||
      envelope.t === "workspace_role_create" ||
      envelope.t === "workspace_role_update" ||
      envelope.t === "workspace_role_delete" ||
      envelope.t === "workspace_role_reorder" ||
      envelope.t === "workspace_role_assignment_add" ||
      envelope.t === "workspace_role_assignment_remove" ||
      envelope.t === "workspace_channel_override_update" ||
      envelope.t === "workspace_ip_ban_sync"
    ) {
      const workspaceEvent = decodeWorkspaceGatewayEvent(envelope.t, envelope.d);
      if (!workspaceEvent) {
        return;
      }

      if (workspaceEvent.type === "channel_create") {
        handlers.onChannelCreate?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_update") {
        handlers.onWorkspaceUpdate?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_member_add") {
        handlers.onWorkspaceMemberAdd?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_member_update") {
        handlers.onWorkspaceMemberUpdate?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_member_remove") {
        handlers.onWorkspaceMemberRemove?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_member_ban") {
        handlers.onWorkspaceMemberBan?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_create") {
        handlers.onWorkspaceRoleCreate?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_update") {
        handlers.onWorkspaceRoleUpdate?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_delete") {
        handlers.onWorkspaceRoleDelete?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_reorder") {
        handlers.onWorkspaceRoleReorder?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_assignment_add") {
        handlers.onWorkspaceRoleAssignmentAdd?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_role_assignment_remove") {
        handlers.onWorkspaceRoleAssignmentRemove?.(workspaceEvent.payload);
        return;
      }
      if (workspaceEvent.type === "workspace_channel_override_update") {
        handlers.onWorkspaceChannelOverrideUpdate?.(workspaceEvent.payload);
        return;
      }
      handlers.onWorkspaceIpBanSync?.(workspaceEvent.payload);
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
