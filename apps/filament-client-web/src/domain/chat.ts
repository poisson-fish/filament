import { DomainValidationError } from "./auth";

export type GuildId = string & { readonly __brand: "guild_id" };
export type ChannelId = string & { readonly __brand: "channel_id" };
export type MessageId = string & { readonly __brand: "message_id" };
export type UserId = string & { readonly __brand: "user_id" };
export type AttachmentId = string & { readonly __brand: "attachment_id" };
export type GuildName = string & { readonly __brand: "guild_name" };
export type ChannelName = string & { readonly __brand: "channel_name" };
export type MessageContent = string & { readonly __brand: "message_content" };
export type SearchQuery = string & { readonly __brand: "search_query" };
export type ReactionEmoji = string & { readonly __brand: "reaction_emoji" };
export type AttachmentFilename = string & { readonly __brand: "attachment_filename" };
export type LivekitToken = string & { readonly __brand: "livekit_token" };
export type LivekitUrl = string & { readonly __brand: "livekit_url" };
export type LivekitRoom = string & { readonly __brand: "livekit_room" };
export type LivekitIdentity = string & { readonly __brand: "livekit_identity" };
export type RoleName = "owner" | "moderator" | "member";
export type PermissionName =
  | "manage_roles"
  | "manage_channel_overrides"
  | "delete_message"
  | "ban_member"
  | "create_message"
  | "publish_video"
  | "publish_screen_share"
  | "subscribe_streams";
export type MediaPublishSource = "microphone" | "camera" | "screen_share";

export type MarkdownToken =
  | { type: "paragraph_start" }
  | { type: "paragraph_end" }
  | { type: "emphasis_start" }
  | { type: "emphasis_end" }
  | { type: "strong_start" }
  | { type: "strong_end" }
  | { type: "list_start"; ordered: boolean }
  | { type: "list_end" }
  | { type: "list_item_start" }
  | { type: "list_item_end" }
  | { type: "link_start"; href: string }
  | { type: "link_end" }
  | { type: "text"; text: string }
  | { type: "code"; code: string }
  | { type: "soft_break" }
  | { type: "hard_break" };

const ULID_PATTERN = /^[0-9A-HJKMNP-TV-Z]{26}$/;
const MAX_MARKDOWN_TOKENS = 4096;
const MAX_MARKDOWN_INLINE_CHARS = 4096;
const MAX_LIVEKIT_TEXT_CHARS = 512;
const MAX_LIVEKIT_TOKEN_CHARS = 8192;

function idFromInput<T extends GuildId | ChannelId | MessageId | UserId | AttachmentId>(
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

function requireObject(dto: unknown, label: string): Record<string, unknown> {
  if (!dto || typeof dto !== "object") {
    throw new DomainValidationError(`Invalid ${label} payload.`);
  }
  return dto as Record<string, unknown>;
}

function requireString(value: unknown, label: string, maxLen = 4096): string {
  if (typeof value !== "string" || value.length > maxLen) {
    throw new DomainValidationError(`${label} must be a string.`);
  }
  return value;
}

function requirePositiveInteger(value: unknown, label: string): number {
  if (!Number.isInteger(value) || (value as number) <= 0) {
    throw new DomainValidationError(`${label} must be a positive integer.`);
  }
  return value as number;
}

function requireNonNegativeInteger(value: unknown, label: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) {
    throw new DomainValidationError(`${label} must be a non-negative integer.`);
  }
  return value as number;
}

function requireBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new DomainValidationError(`${label} must be a boolean.`);
  }
  return value;
}

function requireStringArray(value: unknown, label: string, maxLen = 256): string[] {
  if (!Array.isArray(value)) {
    throw new DomainValidationError(`${label} must be an array.`);
  }
  return value.map((entry, index) => requireString(entry, `${label}[${index}]`, maxLen));
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

export function userIdFromInput(input: string): UserId {
  return idFromInput<UserId>(input, "User ID");
}

export function attachmentIdFromInput(input: string): AttachmentId {
  return idFromInput<AttachmentId>(input, "Attachment ID");
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

export function attachmentFilenameFromInput(input: string): AttachmentFilename {
  if (input.length < 1 || input.length > 128) {
    throw new DomainValidationError("Attachment filename must be 1-128 characters.");
  }
  if (input.includes("/") || input.includes("\\") || input.includes("\0")) {
    throw new DomainValidationError("Attachment filename contains invalid characters.");
  }
  if (input.trim().length === 0) {
    throw new DomainValidationError("Attachment filename must include visible characters.");
  }
  return input as AttachmentFilename;
}

export function roleFromInput(input: string): RoleName {
  if (input !== "owner" && input !== "moderator" && input !== "member") {
    throw new DomainValidationError("Role must be owner, moderator, or member.");
  }
  return input;
}

export function permissionFromInput(input: string): PermissionName {
  if (
    input !== "manage_roles" &&
    input !== "manage_channel_overrides" &&
    input !== "delete_message" &&
    input !== "ban_member" &&
    input !== "create_message" &&
    input !== "publish_video" &&
    input !== "publish_screen_share" &&
    input !== "subscribe_streams"
  ) {
    throw new DomainValidationError("Invalid permission.");
  }
  return input;
}

export function mediaPublishSourceFromInput(input: string): MediaPublishSource {
  if (input !== "microphone" && input !== "camera" && input !== "screen_share") {
    throw new DomainValidationError("Invalid media publish source.");
  }
  return input;
}

function livekitTextFromInput<
  T extends LivekitUrl | LivekitRoom | LivekitIdentity,
>(input: string, label: string): T {
  if (input.length < 1 || input.length > MAX_LIVEKIT_TEXT_CHARS) {
    throw new DomainValidationError(`${label} has invalid length.`);
  }
  return input as T;
}

function livekitTokenFromInput(input: string): LivekitToken {
  if (input.length < 1 || input.length > MAX_LIVEKIT_TOKEN_CHARS) {
    throw new DomainValidationError("LiveKit token has invalid length.");
  }
  for (const char of input) {
    const code = char.charCodeAt(0);
    if (code < 0x21 || code > 0x7e) {
      throw new DomainValidationError("LiveKit token has invalid charset.");
    }
  }
  return input as LivekitToken;
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
  authorId: UserId;
  content: MessageContent;
  markdownTokens: MarkdownToken[];
  createdAtUnix: number;
}

function markdownTokenFromResponse(dto: unknown): MarkdownToken {
  const data = requireObject(dto, "markdown token");
  const type = requireString(data.type, "type", 64);
  if (type === "list_start") {
    return {
      type,
      ordered: requireBoolean(data.ordered, "ordered"),
    };
  }
  if (type === "link_start") {
    return {
      type,
      href: requireString(data.href, "href", MAX_MARKDOWN_INLINE_CHARS),
    };
  }
  if (type === "text") {
    return {
      type,
      text: requireString(data.text, "text", MAX_MARKDOWN_INLINE_CHARS),
    };
  }
  if (type === "code") {
    return {
      type,
      code: requireString(data.code, "code", MAX_MARKDOWN_INLINE_CHARS),
    };
  }
  if (
    type === "paragraph_start" ||
    type === "paragraph_end" ||
    type === "emphasis_start" ||
    type === "emphasis_end" ||
    type === "strong_start" ||
    type === "strong_end" ||
    type === "list_end" ||
    type === "list_item_start" ||
    type === "list_item_end" ||
    type === "link_end" ||
    type === "soft_break" ||
    type === "hard_break"
  ) {
    return { type };
  }
  throw new DomainValidationError("Unsupported markdown token.");
}

export function markdownTokensFromResponse(dto: unknown): MarkdownToken[] {
  if (!Array.isArray(dto) || dto.length > MAX_MARKDOWN_TOKENS) {
    throw new DomainValidationError("markdown_tokens must be a bounded array.");
  }
  return dto.map((entry) => markdownTokenFromResponse(entry));
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
    authorId: userIdFromInput(requireString(data.author_id, "author_id")),
    content: messageContentFromInput(requireString(data.content, "content")),
    markdownTokens: markdownTokensFromResponse(data.markdown_tokens),
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
  return {
    emoji: reactionEmojiFromInput(requireString(data.emoji, "emoji")),
    count: requireNonNegativeInteger(data.count, "count"),
  };
}

export interface ModerationResult {
  accepted: true;
}

export function moderationResultFromResponse(dto: unknown): ModerationResult {
  const data = requireObject(dto, "moderation result");
  if (data.accepted !== true) {
    throw new DomainValidationError("Moderation response must be accepted=true.");
  }
  return { accepted: true };
}

export interface SearchReconcileResult {
  upserted: number;
  deleted: number;
}

export function searchReconcileFromResponse(dto: unknown): SearchReconcileResult {
  const data = requireObject(dto, "search reconcile");
  return {
    upserted: requireNonNegativeInteger(data.upserted, "upserted"),
    deleted: requireNonNegativeInteger(data.deleted, "deleted"),
  };
}

export interface AttachmentRecord {
  attachmentId: AttachmentId;
  guildId: GuildId;
  channelId: ChannelId;
  ownerId: UserId;
  filename: AttachmentFilename;
  mimeType: string;
  sizeBytes: number;
  sha256Hex: string;
}

export function attachmentFromResponse(dto: unknown): AttachmentRecord {
  const data = requireObject(dto, "attachment");
  const mimeType = requireString(data.mime_type, "mime_type", 128);
  const sha256Hex = requireString(data.sha256_hex, "sha256_hex", 128);
  if (!/^[0-9a-f]{64}$/i.test(sha256Hex)) {
    throw new DomainValidationError("sha256_hex must be 64 hex characters.");
  }
  const sizeBytes = requirePositiveInteger(data.size_bytes, "size_bytes");
  return {
    attachmentId: attachmentIdFromInput(requireString(data.attachment_id, "attachment_id")),
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    channelId: channelIdFromInput(requireString(data.channel_id, "channel_id")),
    ownerId: userIdFromInput(requireString(data.owner_id, "owner_id")),
    filename: attachmentFilenameFromInput(requireString(data.filename, "filename", 128)),
    mimeType,
    sizeBytes,
    sha256Hex,
  };
}

export interface VoiceTokenRecord {
  token: LivekitToken;
  livekitUrl: LivekitUrl;
  room: LivekitRoom;
  identity: LivekitIdentity;
  canPublish: boolean;
  canSubscribe: boolean;
  publishSources: MediaPublishSource[];
  expiresInSecs: number;
}

export function voiceTokenFromResponse(dto: unknown): VoiceTokenRecord {
  const data = requireObject(dto, "voice token");
  return {
    token: livekitTokenFromInput(requireString(data.token, "token", MAX_LIVEKIT_TOKEN_CHARS)),
    livekitUrl: livekitTextFromInput<LivekitUrl>(requireString(data.livekit_url, "livekit_url"), "livekit_url"),
    room: livekitTextFromInput<LivekitRoom>(requireString(data.room, "room"), "room"),
    identity: livekitTextFromInput<LivekitIdentity>(requireString(data.identity, "identity"), "identity"),
    canPublish: requireBoolean(data.can_publish, "can_publish"),
    canSubscribe: requireBoolean(data.can_subscribe, "can_subscribe"),
    publishSources: requireStringArray(data.publish_sources, "publish_sources", 32).map((entry) =>
      mediaPublishSourceFromInput(entry),
    ),
    expiresInSecs: requirePositiveInteger(data.expires_in_secs, "expires_in_secs"),
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
