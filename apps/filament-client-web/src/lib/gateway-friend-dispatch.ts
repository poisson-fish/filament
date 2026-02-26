import type {
  FriendRemovePayload,
  FriendRequestCreatePayload,
  FriendRequestDeletePayload,
  FriendRequestUpdatePayload,
} from "./gateway-contracts";
import {
  decodeFriendGatewayEvent,
} from "./gateway-friend-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface FriendGatewayDispatchHandlers {
  onFriendRequestCreate?: (payload: FriendRequestCreatePayload) => void;
  onFriendRequestUpdate?: (payload: FriendRequestUpdatePayload) => void;
  onFriendRequestDelete?: (payload: FriendRequestDeletePayload) => void;
  onFriendRemove?: (payload: FriendRemovePayload) => void;
}

export const FRIEND_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "friend_request_create",
  "friend_request_update",
  "friend_request_delete",
  "friend_remove",
];

const FRIEND_GATEWAY_EVENT_TYPE_SET = new Set<string>(
  FRIEND_GATEWAY_DISPATCH_EVENT_TYPES,
);

type FriendGatewayEvent = NonNullable<ReturnType<typeof decodeFriendGatewayEvent>>;

const FRIEND_DISPATCH_TABLE: GatewayDispatchTable<
  FriendGatewayEvent,
  FriendGatewayDispatchHandlers
> = {
  friend_request_create: (eventPayload, eventHandlers) => {
    eventHandlers.onFriendRequestCreate?.(eventPayload);
  },
  friend_request_update: (eventPayload, eventHandlers) => {
    eventHandlers.onFriendRequestUpdate?.(eventPayload);
  },
  friend_request_delete: (eventPayload, eventHandlers) => {
    eventHandlers.onFriendRequestDelete?.(eventPayload);
  },
  friend_remove: (eventPayload, eventHandlers) => {
    eventHandlers.onFriendRemove?.(eventPayload);
  },
};

export function dispatchFriendGatewayEvent(
  type: string,
  payload: unknown,
  handlers: FriendGatewayDispatchHandlers,
): boolean {
  if (!FRIEND_GATEWAY_EVENT_TYPE_SET.has(type)) {
    return false;
  }

  const friendEvent = decodeFriendGatewayEvent(type, payload);
  if (!friendEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(friendEvent, handlers, FRIEND_DISPATCH_TABLE);
  return true;
}
