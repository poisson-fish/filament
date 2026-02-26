import {
  channelIdFromInput,
  guildIdFromInput,
  userIdFromInput,
} from "../domain/chat";

export interface ReadyPayload {
  userId: string;
}

export interface SubscribedPayload {
  guildId: string;
  channelId: string;
}

export interface ReadyGatewayDispatchHandlers {
  onReady?: (payload: ReadyPayload) => void;
  onSubscribed?: (payload: SubscribedPayload) => void;
}

export const READY_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "ready",
  "subscribed",
];

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

function parseSubscribedPayload(payload: unknown): SubscribedPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }

  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string"
    || typeof value.channel_id !== "string"
  ) {
    return null;
  }

  let guildId: string;
  let channelId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
  } catch {
    return null;
  }

  return { guildId, channelId };
}

export function dispatchReadyGatewayEvent(
  type: string,
  payload: unknown,
  handlers: ReadyGatewayDispatchHandlers,
): boolean {
  if (type !== "ready") {
    return false;
  }

  const readyPayload = parseReadyPayload(payload);
  if (!readyPayload) {
    return true;
  }

  handlers.onReady?.(readyPayload);
  return true;
}

export function dispatchSubscribedGatewayEvent(
  type: string,
  payload: unknown,
  handlers: ReadyGatewayDispatchHandlers,
): boolean {
  if (type !== "subscribed") {
    return false;
  }

  const subscribedPayload = parseSubscribedPayload(payload);
  if (!subscribedPayload) {
    return true;
  }

  handlers.onSubscribed?.(subscribedPayload);
  return true;
}
