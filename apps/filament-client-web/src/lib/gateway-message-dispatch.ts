import type {
  MessageDeletePayload,
  MessageReactionPayload,
  MessageUpdatePayload,
} from "./gateway-contracts";
import {
  decodeMessageGatewayEvent,
} from "./gateway-message-events";
import type {
  MessageRecord,
} from "../domain/chat";

export interface MessageGatewayDispatchHandlers {
  onMessageCreate?: (message: MessageRecord) => void;
  onMessageUpdate?: (payload: MessageUpdatePayload) => void;
  onMessageDelete?: (payload: MessageDeletePayload) => void;
  onMessageReaction?: (payload: MessageReactionPayload) => void;
}

const MESSAGE_GATEWAY_EVENT_TYPES = new Set<string>([
  "message_create",
  "message_update",
  "message_delete",
  "message_reaction",
]);

export function dispatchMessageGatewayEvent(
  type: string,
  payload: unknown,
  handlers: MessageGatewayDispatchHandlers,
): boolean {
  if (!MESSAGE_GATEWAY_EVENT_TYPES.has(type)) {
    return false;
  }

  const messageEvent = decodeMessageGatewayEvent(type, payload);
  if (!messageEvent) {
    return true;
  }

  if (messageEvent.type === "message_create") {
    handlers.onMessageCreate?.(messageEvent.payload);
    return true;
  }
  if (messageEvent.type === "message_update") {
    handlers.onMessageUpdate?.(messageEvent.payload);
    return true;
  }
  if (messageEvent.type === "message_delete") {
    handlers.onMessageDelete?.(messageEvent.payload);
    return true;
  }

  handlers.onMessageReaction?.(messageEvent.payload);
  return true;
}