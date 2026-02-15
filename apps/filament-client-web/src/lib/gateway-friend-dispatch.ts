import type {
  FriendRemovePayload,
  FriendRequestCreatePayload,
  FriendRequestDeletePayload,
  FriendRequestUpdatePayload,
} from "./gateway-contracts";
import {
  decodeFriendGatewayEvent,
} from "./gateway-friend-events";

export interface FriendGatewayDispatchHandlers {
  onFriendRequestCreate?: (payload: FriendRequestCreatePayload) => void;
  onFriendRequestUpdate?: (payload: FriendRequestUpdatePayload) => void;
  onFriendRequestDelete?: (payload: FriendRequestDeletePayload) => void;
  onFriendRemove?: (payload: FriendRemovePayload) => void;
}

const FRIEND_GATEWAY_EVENT_TYPES = new Set<string>([
  "friend_request_create",
  "friend_request_update",
  "friend_request_delete",
  "friend_remove",
]);

export function dispatchFriendGatewayEvent(
  type: string,
  payload: unknown,
  handlers: FriendGatewayDispatchHandlers,
): boolean {
  if (!FRIEND_GATEWAY_EVENT_TYPES.has(type)) {
    return false;
  }

  const friendEvent = decodeFriendGatewayEvent(type, payload);
  if (!friendEvent) {
    return true;
  }

  if (friendEvent.type === "friend_request_create") {
    handlers.onFriendRequestCreate?.(friendEvent.payload);
    return true;
  }
  if (friendEvent.type === "friend_request_update") {
    handlers.onFriendRequestUpdate?.(friendEvent.payload);
    return true;
  }
  if (friendEvent.type === "friend_request_delete") {
    handlers.onFriendRequestDelete?.(friendEvent.payload);
    return true;
  }

  handlers.onFriendRemove?.(friendEvent.payload);
  return true;
}
