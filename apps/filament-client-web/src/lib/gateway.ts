import { type AccessToken } from "../domain/auth";
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
  type RoleName,
  guildIdFromInput,
  messageIdFromInput,
  messageFromResponse,
  reactionEmojiFromInput,
  userIdFromInput,
} from "../domain/chat";

const MAX_GATEWAY_EVENT_BYTES = 64 * 1024;
const MAX_PRESENCE_SYNC_USER_IDS = 1024;
const EVENT_TYPE_PATTERN = /^[a-z0-9_.]{1,64}$/;

type GatewayEventEnvelope = {
  v: number;
  t: string;
  d: unknown;
};

type PresenceStatus = "online" | "offline";

interface ReadyPayload {
  userId: string;
}

interface PresenceSyncPayload {
  guildId: GuildId;
  userIds: string[];
}

interface PresenceUpdatePayload {
  guildId: GuildId;
  userId: string;
  status: PresenceStatus;
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
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
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

function parsePresenceSyncPayload(payload: unknown): PresenceSyncPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string" || !Array.isArray(value.user_ids)) {
    return null;
  }
  if (value.user_ids.length > MAX_PRESENCE_SYNC_USER_IDS) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const seen = new Set<string>();
  const userIds: string[] = [];
  for (const entry of value.user_ids) {
    if (typeof entry !== "string") {
      return null;
    }

    let userId: string;
    try {
      userId = userIdFromInput(entry);
    } catch {
      return null;
    }

    if (seen.has(userId)) {
      continue;
    }
    seen.add(userId);
    userIds.push(userId);
  }

  return {
    guildId,
    userIds,
  };
}

function parsePresenceUpdatePayload(payload: unknown): PresenceUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    (value.status !== "online" && value.status !== "offline")
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
    status: value.status,
  };
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

function parseEnvelope(raw: string): GatewayEventEnvelope | null {
  if (new TextEncoder().encode(raw).length > MAX_GATEWAY_EVENT_BYTES) {
    return null;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }

  if (!parsed || typeof parsed !== "object") {
    return null;
  }

  const value = parsed as Record<string, unknown>;
  if (value.v !== 1 || typeof value.t !== "string" || !EVENT_TYPE_PATTERN.test(value.t)) {
    return null;
  }

  return {
    v: 1,
    t: value.t,
    d: value.d,
  };
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

  const socket = new WebSocket(resolveGatewayUrl(accessToken));
  let currentGuildId = guildId;
  let currentChannelId = channelId;

  socket.onopen = () => {
    handlers.onOpenStateChange?.(true);
    sendEnvelope(socket, "subscribe", {
      guild_id: currentGuildId,
      channel_id: currentChannelId,
    });
  };

  socket.onclose = () => {
    handlers.onOpenStateChange?.(false);
  };

  socket.onmessage = (event) => {
    if (typeof event.data !== "string") {
      return;
    }
    const envelope = parseEnvelope(event.data);
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

    if (envelope.t === "presence_sync") {
      const payload = parsePresenceSyncPayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onPresenceSync?.(payload);
      return;
    }

    if (envelope.t === "presence_update") {
      const payload = parsePresenceUpdatePayload(envelope.d);
      if (!payload) {
        return;
      }
      handlers.onPresenceUpdate?.(payload);
    }
  };

  return {
    updateSubscription: (nextGuildId, nextChannelId) => {
      currentGuildId = nextGuildId;
      currentChannelId = nextChannelId;
      sendEnvelope(socket, "subscribe", {
        guild_id: currentGuildId,
        channel_id: currentChannelId,
      });
    },
    close: () => {
      if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) {
        socket.close();
      }
    },
  };
}
