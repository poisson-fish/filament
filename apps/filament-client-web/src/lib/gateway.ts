import { type AccessToken } from "../domain/auth";
import {
  type ChannelId,
  type GuildId,
  type MessageRecord,
  messageFromResponse,
} from "../domain/chat";

const MAX_GATEWAY_EVENT_BYTES = 64 * 1024;
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

interface GatewayHandlers {
  onReady?: () => void;
  onMessageCreate?: (message: MessageRecord) => void;
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
  onOpenStateChange?: (isOpen: boolean) => void;
}

interface GatewayClient {
  updateSubscription: (guildId: GuildId, channelId: ChannelId) => void;
  close: () => void;
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

    if (envelope.t === "presence_sync") {
      const payload = envelope.d as { guild_id?: unknown; user_ids?: unknown };
      if (typeof payload?.guild_id !== "string" || !Array.isArray(payload.user_ids)) {
        return;
      }
      if (!payload.user_ids.every((entry) => typeof entry === "string")) {
        return;
      }
      handlers.onPresenceSync?.({
        guildId: payload.guild_id as GuildId,
        userIds: payload.user_ids,
      });
      return;
    }

    if (envelope.t === "presence_update") {
      const payload = envelope.d as {
        guild_id?: unknown;
        user_id?: unknown;
        status?: unknown;
      };
      if (
        typeof payload?.guild_id !== "string" ||
        typeof payload?.user_id !== "string" ||
        (payload.status !== "online" && payload.status !== "offline")
      ) {
        return;
      }
      handlers.onPresenceUpdate?.({
        guildId: payload.guild_id as GuildId,
        userId: payload.user_id,
        status: payload.status,
      });
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
