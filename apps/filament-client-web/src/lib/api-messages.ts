import {
  type AccessToken,
  type AuthSession,
} from "../domain/auth";
import {
  type AttachmentId,
  type ChannelId,
  type GuildId,
  type MessageContent,
  type MessageHistory,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
  type ReactionRecord,
  messageFromResponse,
  messageHistoryFromResponse,
  reactionFromResponse,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface MessagesApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  requestNoContent: (request: JsonRequest) => Promise<void>;
  createApiError: (status: number, code: string, message: string) => Error;
  isApiErrorCode: (error: unknown, code: string) => boolean;
}

export interface MessagesApi {
  fetchChannelMessages(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    input?: { limit?: number; before?: MessageId },
  ): Promise<MessageHistory>;
  createChannelMessage(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    input: { content: MessageContent; attachmentIds?: AttachmentId[] },
  ): Promise<MessageRecord>;
  editChannelMessage(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    messageId: MessageId,
    input: { content: MessageContent },
  ): Promise<MessageRecord>;
  deleteChannelMessage(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    messageId: MessageId,
  ): Promise<void>;
  addMessageReaction(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    messageId: MessageId,
    emoji: ReactionEmoji,
  ): Promise<ReactionRecord>;
  removeMessageReaction(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    messageId: MessageId,
    emoji: ReactionEmoji,
  ): Promise<ReactionRecord>;
}

export function createMessagesApi(input: MessagesApiDependencies): MessagesApi {
  return {
    async fetchChannelMessages(session, guildId, channelId, filters) {
      const params = new URLSearchParams();
      if (
        filters?.limit &&
        Number.isInteger(filters.limit) &&
        filters.limit > 0 &&
        filters.limit <= 100
      ) {
        params.set("limit", String(filters.limit));
      }
      if (filters?.before) {
        params.set("before", filters.before);
      }

      const query = params.size > 0 ? `?${params.toString()}` : "";
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/channels/${channelId}/messages${query}`,
        accessToken: session.accessToken,
      });
      return messageHistoryFromResponse(dto);
    },

    async createChannelMessage(session, guildId, channelId, payload) {
      try {
        const dto = await input.requestJson({
          method: "POST",
          path: `/guilds/${guildId}/channels/${channelId}/messages`,
          accessToken: session.accessToken,
          body: {
            content: payload.content,
            attachment_ids: payload.attachmentIds,
          },
        });
        return messageFromResponse(dto);
      } catch (error) {
        if (
          input.isApiErrorCode(error, "invalid_json") &&
          payload.attachmentIds &&
          payload.attachmentIds.length > 0
        ) {
          throw input.createApiError(
            400,
            "protocol_mismatch",
            "Server does not support attachment_ids on message create.",
          );
        }
        throw error;
      }
    },

    async editChannelMessage(session, guildId, channelId, messageId, payload) {
      const dto = await input.requestJson({
        method: "PATCH",
        path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}`,
        accessToken: session.accessToken,
        body: { content: payload.content },
      });
      return messageFromResponse(dto);
    },

    async deleteChannelMessage(session, guildId, channelId, messageId) {
      await input.requestNoContent({
        method: "DELETE",
        path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}`,
        accessToken: session.accessToken,
      });
    },

    async addMessageReaction(session, guildId, channelId, messageId, emoji) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
        accessToken: session.accessToken,
      });
      return reactionFromResponse(dto);
    },

    async removeMessageReaction(session, guildId, channelId, messageId, emoji) {
      const dto = await input.requestJson({
        method: "DELETE",
        path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
        accessToken: session.accessToken,
      });
      return reactionFromResponse(dto);
    },
  };
}
