import { DomainValidationError } from "./auth";

export type GuildId = string & { readonly __brand: "guild_id" };
export type ChannelId = string & { readonly __brand: "channel_id" };
export type MessageId = string & { readonly __brand: "message_id" };
export type GuildName = string & { readonly __brand: "guild_name" };
export type ChannelName = string & { readonly __brand: "channel_name" };
export type MessageContent = string & { readonly __brand: "message_content" };
export type SearchQuery = string & { readonly __brand: "search_query" };
export type ReactionEmoji = string & { readonly __brand: "reaction_emoji" };

const ULID_PATTERN = /^[0-9A-HJKMNP-TV-Z]{26}$/;

function idFromInput<T extends GuildId | ChannelId | MessageId>(
  input: string,
  label: string,
): T {
  if (!ULID_PATTERN.test(input)) {
    throw new DomainValidationError(`${label} must be a valid ULID.`);
  }
  return input as T;
}

function visibleNameFromInput<T extends GuildName | ChannelName>(
  input: string,
  label: string,
): T {
  const value = input.trim();
  if (value.length < 1 || value.length > 64) {
    throw new DomainValidationError(`${label} must be 1-64 characters.`);
  }
  for (const char of value) {
    const code = char.charCodeAt(0);
    if (code < 0x20 || code === 0x7f) {
      throw new DomainValidationError(`${label} contains invalid characters.`);
    }
  }
  return value as T;
}

export function guildIdFromInput(input: string): GuildId {
  return idFromInput<GuildId>(input, "Guild ID");
}

export function channelIdFromInput(input: string): ChannelId {
  return idFromInput<ChannelId>(input, "Channel ID");
}

export function messageIdFromInput(input: string): MessageId {
  return idFromInput<MessageId>(input, "Message ID");
}

export function guildNameFromInput(input: string): GuildName {
  return visibleNameFromInput<GuildName>(input, "Guild name");
}

export function channelNameFromInput(input: string): ChannelName {
  return visibleNameFromInput<ChannelName>(input, "Channel name");
}

export function messageContentFromInput(input: string): MessageContent {
  if (input.length < 1 || input.length > 2000) {
    throw new DomainValidationError("Message content must be 1-2000 characters.");
  }
  return input as MessageContent;
}

export function searchQueryFromInput(input: string): SearchQuery {
  const value = input.trim();
  if (value.length < 1 || value.length > 256) {
    throw new DomainValidationError("Search query must be 1-256 characters.");
  }
  if (value.includes(":")) {
    throw new DomainValidationError("Search query cannot contain ':'.");
  }
  return value as SearchQuery;
}

export function reactionEmojiFromInput(input: string): ReactionEmoji {
  if (input.length < 1 || input.length > 32) {
    throw new DomainValidationError("Reaction emoji must be 1-32 characters.");
  }
  for (const char of input) {
    if (/\s/.test(char)) {
      throw new DomainValidationError("Reaction emoji cannot contain whitespace.");
    }
  }
  return input as ReactionEmoji;
}

export interface GuildRecord {
  guildId: GuildId;
  name: GuildName;
}

export interface ChannelRecord {
  channelId: ChannelId;
  name: ChannelName;
}

export interface MessageRecord {
  messageId: MessageId;
  guildId: GuildId;
  channelId: ChannelId;
  authorId: string;
  content: MessageContent;
  createdAtUnix: number;
}

function requireObject(dto: unknown, label: string): Record<string, unknown> {
  if (!dto || typeof dto !== "object") {
    throw new DomainValidationError(`Invalid ${label} payload.`);
  }
  return dto as Record<string, unknown>;
}

function requirePositiveInteger(value: unknown, label: string): number {
  if (!Number.isInteger(value) || (value as number) <= 0) {
    throw new DomainValidationError(`${label} must be a positive integer.`);
  }
  return value as number;
}

function requireString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new DomainValidationError(`${label} must be a string.`);
  }
  return value;
}

export function guildFromResponse(dto: unknown): GuildRecord {
  const data = requireObject(dto, "guild");
  return {
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    name: guildNameFromInput(requireString(data.name, "name")),
  };
}

export function channelFromResponse(dto: unknown): ChannelRecord {
  const data = requireObject(dto, "channel");
  return {
    channelId: channelIdFromInput(requireString(data.channel_id, "channel_id")),
    name: channelNameFromInput(requireString(data.name, "name")),
  };
}

export function messageFromResponse(dto: unknown): MessageRecord {
  const data = requireObject(dto, "message");
  return {
    messageId: messageIdFromInput(requireString(data.message_id, "message_id")),
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    channelId: channelIdFromInput(requireString(data.channel_id, "channel_id")),
    authorId: requireString(data.author_id, "author_id"),
    content: messageContentFromInput(requireString(data.content, "content")),
    createdAtUnix: requirePositiveInteger(data.created_at_unix, "created_at_unix"),
  };
}

export interface MessageHistory {
  messages: MessageRecord[];
  nextBefore: MessageId | null;
}

export function messageHistoryFromResponse(dto: unknown): MessageHistory {
  const data = requireObject(dto, "message history");
  const messagesDto = data.messages;
  if (!Array.isArray(messagesDto)) {
    throw new DomainValidationError("messages must be an array.");
  }
  const messages = messagesDto.map((entry) => messageFromResponse(entry));
  const nextBeforeRaw = data.next_before;
  const nextBefore =
    nextBeforeRaw === null || typeof nextBeforeRaw === "undefined"
      ? null
      : messageIdFromInput(requireString(nextBeforeRaw, "next_before"));
  return { messages, nextBefore };
}

export interface SearchResults {
  messageIds: MessageId[];
  messages: MessageRecord[];
}

export function searchResultsFromResponse(dto: unknown): SearchResults {
  const data = requireObject(dto, "search");
  if (!Array.isArray(data.message_ids) || !Array.isArray(data.messages)) {
    throw new DomainValidationError("Search response has invalid arrays.");
  }
  return {
    messageIds: data.message_ids.map((id) => messageIdFromInput(requireString(id, "message_id"))),
    messages: data.messages.map((message) => messageFromResponse(message)),
  };
}

export interface ReactionRecord {
  emoji: ReactionEmoji;
  count: number;
}

export function reactionFromResponse(dto: unknown): ReactionRecord {
  const data = requireObject(dto, "reaction");
  const count = data.count;
  if (!Number.isInteger(count) || (count as number) < 0) {
    throw new DomainValidationError("Reaction count must be a non-negative integer.");
  }
  return {
    emoji: reactionEmojiFromInput(requireString(data.emoji, "emoji")),
    count: count as number,
  };
}

export interface WorkspaceRecord {
  guildId: GuildId;
  guildName: GuildName;
  channels: ChannelRecord[];
}

export function workspaceFromStorage(dto: unknown): WorkspaceRecord {
  const data = requireObject(dto, "workspace cache");
  const channelsDto = data.channels;
  if (!Array.isArray(channelsDto)) {
    throw new DomainValidationError("Workspace channels must be an array.");
  }
  return {
    guildId: guildIdFromInput(requireString(data.guildId, "guildId")),
    guildName: guildNameFromInput(requireString(data.guildName, "guildName")),
    channels: channelsDto.map((channel) => {
      const channelObj = requireObject(channel, "channel cache");
      return {
        channelId: channelIdFromInput(requireString(channelObj.channelId, "channelId")),
        name: channelNameFromInput(requireString(channelObj.name, "name")),
      };
    }),
  };
}
