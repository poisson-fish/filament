import type {
  PresenceSyncPayload,
  PresenceUpdatePayload,
} from "./gateway-presence-events";
import {
  decodePresenceGatewayEvent,
} from "./gateway-presence-events";

export interface PresenceGatewayDispatchHandlers {
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
}

const PRESENCE_GATEWAY_EVENT_TYPES = new Set<string>([
  "presence_sync",
  "presence_update",
]);

export function dispatchPresenceGatewayEvent(
  type: string,
  payload: unknown,
  handlers: PresenceGatewayDispatchHandlers,
): boolean {
  if (!PRESENCE_GATEWAY_EVENT_TYPES.has(type)) {
    return false;
  }

  const presenceEvent = decodePresenceGatewayEvent(type, payload);
  if (!presenceEvent) {
    return true;
  }

  if (presenceEvent.type === "presence_sync") {
    handlers.onPresenceSync?.(presenceEvent.payload);
    return true;
  }

  handlers.onPresenceUpdate?.(presenceEvent.payload);
  return true;
}