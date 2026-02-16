import {
  type AccessToken,
  type AuthSession,
} from "../domain/auth";
import {
  type AttachmentId,
  type AttachmentFilename,
  type AttachmentRecord,
  type ChannelId,
  type GuildId,
  type MessageContent,
  type MessageHistory,
  type MessageId,
  type MessageRecord,
  type SearchQuery,
  type SearchReconcileResult,
  type SearchResults,
  type ReactionEmoji,
  type ReactionRecord,
  attachmentFromResponse,
  messageFromResponse,
  messageHistoryFromResponse,
  reactionFromResponse,
  searchReconcileFromResponse,
  searchResultsFromResponse,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface BodyRequest {
  method: "POST" | "PATCH" | "DELETE";
  path: string;
  body: BodyInit;
  accessToken?: AccessToken;
  headers?: Record<string, string>;
}

interface MessagesApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  requestNoContent: (request: JsonRequest) => Promise<void>;
  requestJsonWithBody: (request: BodyRequest) => Promise<unknown>;
  requestBinary: (input: {
    path: string;
    accessToken: AccessToken;
    timeoutMs?: number;
    maxBytes?: number;
  }) => Promise<{ bytes: Uint8Array; mimeType: string | null }>;
  createApiError: (status: number, code: string, message: string) => Error;
  isApiErrorCode: (error: unknown, code: string) => boolean;
}

const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;
const ATTACHMENT_DOWNLOAD_PREVIEW_MAX_BYTES = 12 * 1024 * 1024;

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
  searchGuildMessages(
    session: AuthSession,
    guildId: GuildId,
    input: { query: SearchQuery; limit?: number; channelId?: ChannelId },
  ): Promise<SearchResults>;
  rebuildGuildSearchIndex(
    session: AuthSession,
    guildId: GuildId,
  ): Promise<void>;
  reconcileGuildSearchIndex(
    session: AuthSession,
    guildId: GuildId,
  ): Promise<SearchReconcileResult>;
  uploadChannelAttachment(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    file: File,
    filename: AttachmentFilename,
  ): Promise<AttachmentRecord>;
  downloadChannelAttachment(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    attachmentId: AttachmentId,
  ): Promise<{ bytes: Uint8Array; mimeType: string | null }>;
  downloadChannelAttachmentPreview(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    attachmentId: AttachmentId,
  ): Promise<{ bytes: Uint8Array; mimeType: string | null }>;
  deleteChannelAttachment(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    attachmentId: AttachmentId,
  ): Promise<void>;
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

    async searchGuildMessages(session, guildId, payload) {
      const params = new URLSearchParams();
      params.set("q", payload.query);
      if (
        payload.limit &&
        Number.isInteger(payload.limit) &&
        payload.limit > 0 &&
        payload.limit <= 50
      ) {
        params.set("limit", String(payload.limit));
      }
      if (payload.channelId) {
        params.set("channel_id", payload.channelId);
      }

      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/search?${params.toString()}`,
        accessToken: session.accessToken,
      });
      return searchResultsFromResponse(dto);
    },

    async rebuildGuildSearchIndex(session, guildId) {
      await input.requestNoContent({
        method: "POST",
        path: `/guilds/${guildId}/search/rebuild`,
        accessToken: session.accessToken,
      });
    },

    async reconcileGuildSearchIndex(session, guildId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/search/reconcile`,
        accessToken: session.accessToken,
      });
      return searchReconcileFromResponse(dto);
    },

    async uploadChannelAttachment(session, guildId, channelId, file, filename) {
      if (file.size < 1 || file.size > MAX_ATTACHMENT_BYTES) {
        throw input.createApiError(
          400,
          "invalid_request",
          "Attachment size must be within server limits.",
        );
      }
      const query = new URLSearchParams({ filename });
      const headers: Record<string, string> = {};
      if (file.type && file.type.length <= 128) {
        headers["content-type"] = file.type;
      }
      const dto = await input.requestJsonWithBody({
        method: "POST",
        path: `/guilds/${guildId}/channels/${channelId}/attachments?${query.toString()}`,
        accessToken: session.accessToken,
        headers,
        body: file,
      });
      return attachmentFromResponse(dto);
    },

    async downloadChannelAttachment(session, guildId, channelId, attachmentId) {
      return input.requestBinary({
        path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
        accessToken: session.accessToken,
      });
    },

    async downloadChannelAttachmentPreview(session, guildId, channelId, attachmentId) {
      return input.requestBinary({
        path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
        accessToken: session.accessToken,
        timeoutMs: 15_000,
        maxBytes: ATTACHMENT_DOWNLOAD_PREVIEW_MAX_BYTES,
      });
    },

    async deleteChannelAttachment(session, guildId, channelId, attachmentId) {
      await input.requestNoContent({
        method: "DELETE",
        path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
        accessToken: session.accessToken,
      });
    },
  };
}
