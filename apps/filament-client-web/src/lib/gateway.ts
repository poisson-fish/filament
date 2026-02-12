import { type AccessToken } from "../domain/auth";
import {
  channelFromResponse,
  channelIdFromInput,
  type ChannelRecord,
  type ChannelId,
  type GuildId,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
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

export interface MessageReactionPayload {
  guildId: GuildId;
  channelId: ChannelId;
  messageId: MessageId;
  emoji: ReactionEmoji;
  count: number;
}

interface GatewayHandlers {
  onReady?: () => void;
  onMessageCreate?: (message: MessageRecord) => void;
  onMessageReaction?: (payload: MessageReactionPayload) => void;
  onChannelCreate?: (payload: ChannelCreatePayload) => void;
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
  onOpenStateChange?: (isOpen: boolean) => void;
}

interface GatewayClient {
  updateSubscription: (guildId: GuildId, channelId: ChannelId) => void;
  close: () => void;
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
      handlers.onReady?.();
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
