import { type AuthSession } from "../domain/auth";
import {
  type AttachmentFilename,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelId,
  type GuildId,
  type MessageContent,
  type MessageHistory,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
  type ReactionRecord,
  type SearchQuery,
  type SearchReconcileResult,
  type SearchResults,
} from "../domain/chat";
import type { MessagesApi } from "./api-messages";

interface MessagesClientDependencies {
  messagesApi: MessagesApi;
}

export interface MessagesClient {
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

export function createMessagesClient(input: MessagesClientDependencies): MessagesClient {
  return {
    fetchChannelMessages(session, guildId, channelId, filters) {
      return input.messagesApi.fetchChannelMessages(session, guildId, channelId, filters);
    },

    createChannelMessage(session, guildId, channelId, payload) {
      return input.messagesApi.createChannelMessage(session, guildId, channelId, payload);
    },

    editChannelMessage(session, guildId, channelId, messageId, payload) {
      return input.messagesApi.editChannelMessage(session, guildId, channelId, messageId, payload);
    },

    deleteChannelMessage(session, guildId, channelId, messageId) {
      return input.messagesApi.deleteChannelMessage(session, guildId, channelId, messageId);
    },

    addMessageReaction(session, guildId, channelId, messageId, emoji) {
      return input.messagesApi.addMessageReaction(session, guildId, channelId, messageId, emoji);
    },

    removeMessageReaction(session, guildId, channelId, messageId, emoji) {
      return input.messagesApi.removeMessageReaction(session, guildId, channelId, messageId, emoji);
    },

    searchGuildMessages(session, guildId, payload) {
      return input.messagesApi.searchGuildMessages(session, guildId, payload);
    },

    rebuildGuildSearchIndex(session, guildId) {
      return input.messagesApi.rebuildGuildSearchIndex(session, guildId);
    },

    reconcileGuildSearchIndex(session, guildId) {
      return input.messagesApi.reconcileGuildSearchIndex(session, guildId);
    },

    uploadChannelAttachment(session, guildId, channelId, file, filename) {
      return input.messagesApi.uploadChannelAttachment(session, guildId, channelId, file, filename);
    },

    downloadChannelAttachment(session, guildId, channelId, attachmentId) {
      return input.messagesApi.downloadChannelAttachment(session, guildId, channelId, attachmentId);
    },

    downloadChannelAttachmentPreview(session, guildId, channelId, attachmentId) {
      return input.messagesApi.downloadChannelAttachmentPreview(
        session,
        guildId,
        channelId,
        attachmentId,
      );
    },

    deleteChannelAttachment(session, guildId, channelId, attachmentId) {
      return input.messagesApi.deleteChannelAttachment(session, guildId, channelId, attachmentId);
    },
  };
}
