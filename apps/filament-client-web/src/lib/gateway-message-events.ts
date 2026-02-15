import {
  markdownTokensFromResponse,
  messageContentFromInput,
  messageFromResponse,
  messageIdFromInput,
  reactionEmojiFromInput,
  channelIdFromInput,
  guildIdFromInput,
  type MarkdownToken,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
  type ChannelId,
  type GuildId,
} from "../domain/chat";
import type {
  MessageDeletePayload,
  MessageReactionPayload,
  MessageUpdatePayload,
} from "./gateway-contracts";

type MessageGatewayEvent =
  | {
      type: "message_create";
      payload: MessageRecord;
    }
  | {
      type: "message_update";
      payload: MessageUpdatePayload;
    }
  | {
      type: "message_delete";
      payload: MessageDeletePayload;
    }
  | {
      type: "message_reaction";
      payload: MessageReactionPayload;
    };

type MessageGatewayEventType = MessageGatewayEvent["type"];
type MessageEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseMessageCreatePayload(payload: unknown): MessageRecord | null {
  try {
    return messageFromResponse(payload);
  } catch {
    return null;
  }
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

function parseMessageUpdatePayload(payload: unknown): MessageUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.message_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let messageId: MessageId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    messageId = messageIdFromInput(value.message_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let content: MessageRecord["content"] | undefined;
  let markdownTokens: MarkdownToken[] | undefined;
  if (typeof updatedFieldsDto.content !== "undefined") {
    if (typeof updatedFieldsDto.content !== "string") {
      return null;
    }
    try {
      content = messageContentFromInput(updatedFieldsDto.content);
    } catch {
      return null;
    }
  }
  if (typeof updatedFieldsDto.markdown_tokens !== "undefined") {
    try {
      markdownTokens = markdownTokensFromResponse(updatedFieldsDto.markdown_tokens);
    } catch {
      return null;
    }
  }
  if (typeof content === "undefined" && typeof markdownTokens === "undefined") {
    return null;
  }

  return {
    guildId,
    channelId,
    messageId,
    updatedFields: {
      content,
      markdownTokens,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseMessageDeletePayload(payload: unknown): MessageDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.message_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let messageId: MessageId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    messageId = messageIdFromInput(value.message_id);
  } catch {
    return null;
  }

  return {
    guildId,
    channelId,
    messageId,
    deletedAtUnix: value.deleted_at_unix,
  };
}

const MESSAGE_EVENT_DECODERS: {
  [K in MessageGatewayEventType]: MessageEventDecoder<Extract<MessageGatewayEvent, { type: K }>["payload"]>;
} = {
  message_create: parseMessageCreatePayload,
  message_update: parseMessageUpdatePayload,
  message_delete: parseMessageDeletePayload,
  message_reaction: parseMessageReactionPayload,
};

function isMessageGatewayEventType(value: string): value is MessageGatewayEventType {
  return Object.prototype.hasOwnProperty.call(MESSAGE_EVENT_DECODERS, value);
}

export function decodeMessageGatewayEvent(
  type: string,
  payload: unknown,
): MessageGatewayEvent | null {
  if (!isMessageGatewayEventType(type)) {
    return null;
  }

  if (type === "message_create") {
    const parsedPayload = MESSAGE_EVENT_DECODERS.message_create(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "message_update") {
    const parsedPayload = MESSAGE_EVENT_DECODERS.message_update(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "message_delete") {
    const parsedPayload = MESSAGE_EVENT_DECODERS.message_delete(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  const parsedPayload = MESSAGE_EVENT_DECODERS.message_reaction(payload);
  if (!parsedPayload) {
    return null;
  }
  return {
    type,
    payload: parsedPayload,
  };
}