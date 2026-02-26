import type {
  MessageDeletePayload,
  MessageReactionPayload,
  MessageUpdatePayload,
} from "./gateway-contracts";
import {
  decodeMessageGatewayEvent,
} from "./gateway-message-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";
import type {
  MessageRecord,
} from "../domain/chat";

export interface MessageGatewayDispatchHandlers {
  onMessageCreate?: (message: MessageRecord) => void;
  onMessageUpdate?: (payload: MessageUpdatePayload) => void;
  onMessageDelete?: (payload: MessageDeletePayload) => void;
  onMessageReaction?: (payload: MessageReactionPayload) => void;
}

export const MESSAGE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "message_create",
  "message_update",
  "message_delete",
  "message_reaction",
];

const MESSAGE_GATEWAY_EVENT_TYPE_SET = new Set<string>(
  MESSAGE_GATEWAY_DISPATCH_EVENT_TYPES,
);

type MessageGatewayEvent = NonNullable<
  ReturnType<typeof decodeMessageGatewayEvent>
>;

const MESSAGE_DISPATCH_TABLE: GatewayDispatchTable<
  MessageGatewayEvent,
  MessageGatewayDispatchHandlers
> = {
  message_create: (eventPayload, eventHandlers) => {
    eventHandlers.onMessageCreate?.(eventPayload);
  },
  message_update: (eventPayload, eventHandlers) => {
    eventHandlers.onMessageUpdate?.(eventPayload);
  },
  message_delete: (eventPayload, eventHandlers) => {
    eventHandlers.onMessageDelete?.(eventPayload);
  },
  message_reaction: (eventPayload, eventHandlers) => {
    eventHandlers.onMessageReaction?.(eventPayload);
  },
};

export function dispatchMessageGatewayEvent(
  type: string,
  payload: unknown,
  handlers: MessageGatewayDispatchHandlers,
): boolean {
  if (!MESSAGE_GATEWAY_EVENT_TYPE_SET.has(type)) {
    return false;
  }

  const messageEvent = decodeMessageGatewayEvent(type, payload);
  if (!messageEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(messageEvent, handlers, MESSAGE_DISPATCH_TABLE);
  return true;
}
