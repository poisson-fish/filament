import type {
  PresenceSyncPayload,
  PresenceUpdatePayload,
} from "./gateway-presence-events";
import {
  decodePresenceGatewayEvent,
} from "./gateway-presence-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface PresenceGatewayDispatchHandlers {
  onPresenceSync?: (payload: PresenceSyncPayload) => void;
  onPresenceUpdate?: (payload: PresenceUpdatePayload) => void;
}

export const PRESENCE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "presence_sync",
  "presence_update",
];

const PRESENCE_GATEWAY_EVENT_TYPE_SET = new Set<string>(
  PRESENCE_GATEWAY_DISPATCH_EVENT_TYPES,
);

type PresenceGatewayEvent = NonNullable<
  ReturnType<typeof decodePresenceGatewayEvent>
>;

const PRESENCE_DISPATCH_TABLE: GatewayDispatchTable<
  PresenceGatewayEvent,
  PresenceGatewayDispatchHandlers
> = {
  presence_sync: (eventPayload, eventHandlers) => {
    eventHandlers.onPresenceSync?.(eventPayload);
  },
  presence_update: (eventPayload, eventHandlers) => {
    eventHandlers.onPresenceUpdate?.(eventPayload);
  },
};

export function dispatchPresenceGatewayEvent(
  type: string,
  payload: unknown,
  handlers: PresenceGatewayDispatchHandlers,
): boolean {
  if (!PRESENCE_GATEWAY_EVENT_TYPE_SET.has(type)) {
    return false;
  }

  const presenceEvent = decodePresenceGatewayEvent(type, payload);
  if (!presenceEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(
    presenceEvent,
    handlers,
    PRESENCE_DISPATCH_TABLE,
  );
  return true;
}
