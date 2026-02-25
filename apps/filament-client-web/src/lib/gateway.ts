import { type AccessToken } from "../domain/auth";
import {
  type ChannelId,
  type GuildId,
} from "../domain/chat";
import {
  parseGatewayEventEnvelope,
} from "./gateway-envelope";
import {
  dispatchGatewayDomainEvent,
} from "./gateway-domain-dispatch";
import {
  dispatchSubscribedGatewayEvent,
  dispatchReadyGatewayEvent,
} from "./gateway-ready-dispatch";
import {
  type GatewayHandlers,
} from "./gateway-contracts";

export * from "./gateway-contracts";

interface GatewayClient {
  updateSubscription: (guildId: GuildId, channelId: ChannelId) => void;
  setSubscribedChannels: (guildId: GuildId, channelIds: ReadonlyArray<ChannelId>) => void;
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

function sendEnvelope(socket: WebSocket, type: string, data: unknown): void {
  if (socket.readyState !== WebSocket.OPEN) {
    return;
  }
  socket.send(JSON.stringify({ v: 1, t: type, d: data }));
}

function sendSubscribeEnvelopes(
  socket: WebSocket,
  guildId: GuildId,
  channelIds: ReadonlyArray<ChannelId>,
): void {
  for (const subscribedChannelId of channelIds) {
    sendEnvelope(socket, "subscribe", {
      guild_id: guildId,
      channel_id: subscribedChannelId,
    });
  }
}

function uniqueChannelIds(channelIds: ReadonlyArray<ChannelId>): ChannelId[] {
  const seen = new Set<string>();
  const unique: ChannelId[] = [];
  for (const channelId of channelIds) {
    if (seen.has(channelId)) {
      continue;
    }
    seen.add(channelId);
    unique.push(channelId);
  }
  return unique;
}

function sameChannelList(
  left: ReadonlyArray<ChannelId>,
  right: ReadonlyArray<ChannelId>,
): boolean {
  if (left.length !== right.length) {
    return false;
  }
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }
  return true;
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
      setSubscribedChannels: () => {},
      close: () => {},
    };
  }

  let socket: WebSocket | null = null;
  let currentGuildId = guildId;
  let currentChannelId = channelId;
  let currentSubscribedChannelIds: ChannelId[] = [channelId];
  let isClosed = false;
  let retryDelay = 1000;
  let reconnectTimer: number | null = null;

  const setCurrentSubscriptions = (
    nextGuildId: GuildId,
    nextChannelIds: ReadonlyArray<ChannelId>,
  ): boolean => {
    const deduped = uniqueChannelIds(nextChannelIds);
    const normalized =
      deduped.length > 0
        ? deduped
        : [nextChannelIds[0] ?? currentChannelId ?? channelId];
    const changed =
      nextGuildId !== currentGuildId ||
      !sameChannelList(currentSubscribedChannelIds, normalized);
    currentGuildId = nextGuildId;
    currentChannelId = normalized[0] ?? channelId;
    currentSubscribedChannelIds = normalized;
    return changed;
  };

  const handleMessage = (event: MessageEvent) => {
    if (typeof event.data !== "string") {
      return;
    }
    const envelope = parseGatewayEventEnvelope(event.data);
    if (!envelope) {
      return;
    }

    if (dispatchReadyGatewayEvent(envelope.t, envelope.d, handlers)) {
      return;
    }

    if (dispatchSubscribedGatewayEvent(envelope.t, envelope.d, handlers)) {
      return;
    }

    if (dispatchGatewayDomainEvent(envelope.t, envelope.d, handlers)) {
      return;
    }
  };

  const connect = () => {
    if (isClosed) return;

    socket = new WebSocket(resolveGatewayUrl(accessToken));

    socket.onopen = () => {
      retryDelay = 1000;
      handlers.onOpenStateChange?.(true);
      if (socket && socket.readyState === WebSocket.OPEN) {
        sendSubscribeEnvelopes(
          socket,
          currentGuildId,
          currentSubscribedChannelIds,
        );
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
      const changed = setCurrentSubscriptions(nextGuildId, [nextChannelId]);
      if (!changed) {
        return;
      }
      if (socket && socket.readyState === WebSocket.OPEN) {
        sendSubscribeEnvelopes(
          socket,
          currentGuildId,
          currentSubscribedChannelIds,
        );
      }
    },
    setSubscribedChannels: (nextGuildId, nextChannelIds) => {
      const changed = setCurrentSubscriptions(nextGuildId, nextChannelIds);
      if (!changed) {
        return;
      }
      if (socket && socket.readyState === WebSocket.OPEN) {
        sendSubscribeEnvelopes(
          socket,
          currentGuildId,
          currentSubscribedChannelIds,
        );
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
