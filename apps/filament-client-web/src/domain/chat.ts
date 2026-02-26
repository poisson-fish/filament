import { DomainValidationError, usernameFromInput } from "./auth";

export type GuildId = string & { readonly __brand: "guild_id" };
export type ChannelId = string & { readonly __brand: "channel_id" };
export type MessageId = string & { readonly __brand: "message_id" };
export type UserId = string & { readonly __brand: "user_id" };
export type AttachmentId = string & { readonly __brand: "attachment_id" };
export type FriendRequestId = string & { readonly __brand: "friend_request_id" };
export type GuildIpBanId = string & { readonly __brand: "guild_ip_ban_id" };
export type WorkspaceRoleId = string & { readonly __brand: "workspace_role_id" };
export type GuildName = string & { readonly __brand: "guild_name" };
export type ChannelName = string & { readonly __brand: "channel_name" };
export type WorkspaceRoleName = string & { readonly __brand: "workspace_role_name" };
export type RoleColorHex = string & { readonly __brand: "role_color_hex" };
export type MessageContent = string & { readonly __brand: "message_content" };
export type SearchQuery = string & { readonly __brand: "search_query" };
export type ReactionEmoji = string & { readonly __brand: "reaction_emoji" };
export type AttachmentFilename = string & { readonly __brand: "attachment_filename" };
export type IpNetwork = string & { readonly __brand: "ip_network" };
export type AuditCursor = string & { readonly __brand: "audit_cursor" };
export type LivekitToken = string & { readonly __brand: "livekit_token" };
export type LivekitUrl = string & { readonly __brand: "livekit_url" };
export type LivekitRoom = string & { readonly __brand: "livekit_room" };
export type LivekitIdentity = string & { readonly __brand: "livekit_identity" };
export type GuildVisibility = "private" | "public";
export type ChannelKindName = "text" | "voice";
export type RoleName = "owner" | "moderator" | "member";
export type PermissionName =
  | "manage_roles"
  | "manage_member_roles"
  | "manage_workspace_roles"
  | "manage_channel_overrides"
  | "delete_message"
  | "ban_member"
  | "view_audit_log"
  | "manage_ip_bans"
  | "create_message"
  | "publish_video"
  | "publish_screen_share"
  | "subscribe_streams";
export type MediaPublishSource = "microphone" | "camera" | "screen_share";

export type MarkdownToken =
  | { type: "paragraph_start" }
  | { type: "paragraph_end" }
  | { type: "heading_start"; level: number }
  | { type: "heading_end" }
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
  | { type: "fenced_code"; language: string | null; code: string }
  | { type: "soft_break" }
  | { type: "hard_break" };

const ULID_PATTERN = /^[0-9A-HJKMNP-TV-Z]{26}$/;
const MAX_MARKDOWN_TOKENS = 4096;
const MAX_MARKDOWN_INLINE_CHARS = 4096;
const MAX_MARKDOWN_CODE_BLOCK_CHARS = 16384;
const MAX_MARKDOWN_CODE_BLOCKS = 64;
const MAX_MARKDOWN_CODE_LANGUAGE_CHARS = 32;
export const PROFILE_ABOUT_MAX_CHARS = 2048;
const MAX_LIVEKIT_TEXT_CHARS = 512;
const MAX_LIVEKIT_TOKEN_CHARS = 8192;
const MAX_AUDIT_CURSOR_CHARS = 128;
const MAX_AUDIT_EVENT_ACTION_CHARS = 64;
const MAX_AUDIT_EVENTS_PER_PAGE = 100;
const MAX_GUILD_IP_BANS_PER_PAGE = 100;
const MAX_GUILD_IP_BAN_REASON_CHARS = 240;
const MAX_APPLIED_GUILD_IP_BAN_IDS = 2048;
const MAX_GUILD_MEMBERS_PER_PAGE = 200;
const MAX_WORKSPACE_ROLE_NAME_CHARS = 32;
const ROLE_COLOR_HEX_PATTERN = /^#[0-9A-Fa-f]{6}$/;
const MAX_WORKSPACE_ROLES_PER_GUILD = 64;
const MAX_WORKSPACE_ROLE_PERMISSIONS = 64;
const MAX_WORKSPACE_ROLE_ASSIGNMENTS_PER_USER = 64;
const MAX_REACTIONS_PER_MESSAGE = 64;
const MAX_REACTOR_USER_IDS_PER_REACTION = 32;
const IPV6_MAX_VALUE = (1n << 128n) - 1n;

function idFromInput<
  T extends
  | GuildId
  | ChannelId
  | MessageId
  | UserId
  | AttachmentId
  | FriendRequestId
  | GuildIpBanId
  | WorkspaceRoleId,
>(
  input: string,
  label: string,
): T {
  if (!ULID_PATTERN.test(input)) {
    throw new DomainValidationError(`${label} must be a valid ULID.`);
  }
  return input as T;
}

export function friendRequestIdFromInput(input: string): FriendRequestId {
  return idFromInput<FriendRequestId>(input, "Friend request ID");
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

function parseIpv4Address(value: string): number | null {
  const parts = value.split(".");
  if (parts.length !== 4) {
    return null;
  }
  let address = 0;
  for (const part of parts) {
    if (!/^\d{1,3}$/.test(part)) {
      return null;
    }
    const octet = Number.parseInt(part, 10);
    if (!Number.isInteger(octet) || octet < 0 || octet > 255) {
      return null;
    }
    address = (address << 8) | octet;
  }
  return address >>> 0;
}

function formatIpv4Address(value: number): string {
  return [
    (value >>> 24) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 8) & 0xff,
    value & 0xff,
  ].join(".");
}

function canonicalizeIpv4Address(value: number, prefix: number): number {
  const mask = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0;
  return value & mask;
}

function parseIpv6Address(value: string): bigint | null {
  if (value.includes(".")) {
    return null;
  }

  const collapsed = value.split("::");
  if (collapsed.length > 2) {
    return null;
  }

  const parseSegmentList = (segmentList: string): number[] | null => {
    if (segmentList.length === 0) {
      return [];
    }
    const segments = segmentList.split(":");
    const parsed: number[] = [];
    for (const segment of segments) {
      if (!/^[0-9a-fA-F]{1,4}$/.test(segment)) {
        return null;
      }
      parsed.push(Number.parseInt(segment, 16));
    }
    return parsed;
  };

  const left = parseSegmentList(collapsed[0] ?? "");
  const right = collapsed.length === 2 ? parseSegmentList(collapsed[1] ?? "") : [];
  if (!left || !right) {
    return null;
  }

  let hextets: number[];
  if (collapsed.length === 1) {
    if (left.length !== 8) {
      return null;
    }
    hextets = left;
  } else {
    if (left.length + right.length >= 8) {
      return null;
    }
    const missing = 8 - (left.length + right.length);
    hextets = [...left, ...Array.from({ length: missing }, () => 0), ...right];
  }

  if (hextets.length !== 8) {
    return null;
  }

  let out = 0n;
  for (const hextet of hextets) {
    out = (out << 16n) | BigInt(hextet);
  }
  return out;
}

function formatIpv6Address(value: bigint): string {
  const hextets = Array.from({ length: 8 }, (_unused, index) => {
    const shift = BigInt((7 - index) * 16);
    return Number((value >> shift) & 0xffffn);
  });

  let bestStart = -1;
  let bestLength = 0;
  for (let index = 0; index < hextets.length; index += 1) {
    if (hextets[index] !== 0) {
      continue;
    }
    let cursor = index;
    while (cursor < hextets.length && hextets[cursor] === 0) {
      cursor += 1;
    }
    const runLength = cursor - index;
    if (runLength > bestLength && runLength >= 2) {
      bestStart = index;
      bestLength = runLength;
    }
    index = cursor - 1;
  }

  let rendered = "";
  for (let index = 0; index < hextets.length; index += 1) {
    if (bestStart >= 0 && index === bestStart) {
      rendered += "::";
      index += bestLength - 1;
      continue;
    }
    if (rendered.length > 0 && !rendered.endsWith(":")) {
      rendered += ":";
    }
    rendered += hextets[index]!.toString(16);
  }

  return rendered.length === 0 ? "::" : rendered;
}

function canonicalizeIpv6Address(value: bigint, prefix: number): bigint {
  if (prefix === 0) {
    return 0n;
  }
  const mask = (IPV6_MAX_VALUE << BigInt(128 - prefix)) & IPV6_MAX_VALUE;
  return value & mask;
}

function parseNetworkPrefix(value: string, maxBits: number): number {
  if (!/^\d{1,3}$/.test(value)) {
    throw new DomainValidationError("IP network prefix must be numeric.");
  }
  const prefix = Number.parseInt(value, 10);
  if (!Number.isInteger(prefix) || prefix < 0 || prefix > maxBits) {
    throw new DomainValidationError("IP network prefix is out of range.");
  }
  return prefix;
}

function hasRawIpField(value: Record<string, unknown>): boolean {
  return Object.keys(value).some((key) => {
    const normalized = key.toLowerCase();
    return (
      normalized === "ip" ||
      normalized === "cidr" ||
      normalized.includes("_ip") ||
      normalized.includes("ip_") ||
      normalized.includes("cidr")
    );
  });
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

export function guildIpBanIdFromInput(input: string): GuildIpBanId {
  return idFromInput<GuildIpBanId>(input, "Guild IP ban ID");
}

export function workspaceRoleIdFromInput(input: string): WorkspaceRoleId {
  return idFromInput<WorkspaceRoleId>(input, "Workspace role ID");
}

export function auditCursorFromInput(input: string): AuditCursor {
  if (
    input.length < 1 ||
    input.length > MAX_AUDIT_CURSOR_CHARS ||
    !/^[A-Za-z0-9_-]+$/.test(input)
  ) {
    throw new DomainValidationError("Audit cursor must be 1-128 chars using [A-Za-z0-9_-].");
  }
  return input as AuditCursor;
}

export function ipNetworkFromInput(input: string): IpNetwork {
  const value = input.trim();
  if (value.length === 0) {
    throw new DomainValidationError("IP network must be non-empty.");
  }
  const slashIndex = value.indexOf("/");
  if (slashIndex >= 0 && value.indexOf("/", slashIndex + 1) >= 0) {
    throw new DomainValidationError("IP network must include at most one prefix separator.");
  }

  const addressPart = slashIndex >= 0 ? value.slice(0, slashIndex) : value;
  const prefixPart = slashIndex >= 0 ? value.slice(slashIndex + 1) : null;
  if (addressPart.length === 0 || prefixPart === "") {
    throw new DomainValidationError("IP network must include both address and prefix.");
  }

  const ipv4 = parseIpv4Address(addressPart);
  if (ipv4 !== null) {
    const prefix = prefixPart === null ? 32 : parseNetworkPrefix(prefixPart, 32);
    const canonicalAddress = canonicalizeIpv4Address(ipv4, prefix);
    return `${formatIpv4Address(canonicalAddress)}/${prefix}` as IpNetwork;
  }

  const ipv6 = parseIpv6Address(addressPart);
  if (ipv6 !== null) {
    const prefix = prefixPart === null ? 128 : parseNetworkPrefix(prefixPart, 128);
    const canonicalAddress = canonicalizeIpv6Address(ipv6, prefix);
    return `${formatIpv6Address(canonicalAddress)}/${prefix}` as IpNetwork;
  }

  throw new DomainValidationError("IP network must be a valid IPv4/IPv6 address or CIDR.");
}

export function guildNameFromInput(input: string): GuildName {
  return visibleNameFromInput<GuildName>(input, "Guild name");
}

export function channelNameFromInput(input: string): ChannelName {
  return visibleNameFromInput<ChannelName>(input, "Channel name");
}

export function workspaceRoleNameFromInput(input: string): WorkspaceRoleName {
  const normalized = normalizeWorkspaceRoleName(input);
  if (normalized === "@everyone" || normalized.toLowerCase() === "workspace_owner") {
    throw new DomainValidationError("Workspace role name is reserved.");
  }
  return normalized as WorkspaceRoleName;
}

export function roleColorHexFromInput(input: string): RoleColorHex {
  const normalized = input.trim();
  if (!ROLE_COLOR_HEX_PATTERN.test(normalized)) {
    throw new DomainValidationError("Role color must be a #RRGGBB hex value.");
  }
  return normalized.toUpperCase() as RoleColorHex;
}

function normalizeWorkspaceRoleName(input: string): string {
  const normalized = input.trim();
  if (normalized.length < 1 || normalized.length > MAX_WORKSPACE_ROLE_NAME_CHARS) {
    throw new DomainValidationError("Workspace role name must be 1-32 characters.");
  }
  for (const char of normalized) {
    const code = char.charCodeAt(0);
    if (code < 0x20 || code === 0x7f) {
      throw new DomainValidationError("Workspace role name contains invalid characters.");
    }
  }
  return normalized;
}

function workspaceRoleNameFromResponse(input: unknown): WorkspaceRoleName {
  // Server may return reserved system role names (e.g., @everyone, workspace_owner).
  return normalizeWorkspaceRoleName(
    requireString(input, "name", MAX_WORKSPACE_ROLE_NAME_CHARS),
  ) as WorkspaceRoleName;
}

export function channelKindFromInput(input: string): ChannelKindName {
  if (input !== "text" && input !== "voice") {
    throw new DomainValidationError("Channel kind must be text or voice.");
  }
  return input;
}

export function messageContentFromInput(input: string): MessageContent {
  if (input.length > 2000) {
    throw new DomainValidationError("Message content must be 0-2000 characters.");
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

export function guildVisibilityFromInput(input: string): GuildVisibility {
  if (input !== "private" && input !== "public") {
    throw new DomainValidationError("Guild visibility must be private or public.");
  }
  return input;
}

export function permissionFromInput(input: string): PermissionName {
  if (
    input !== "manage_roles" &&
    input !== "manage_member_roles" &&
    input !== "manage_workspace_roles" &&
    input !== "manage_channel_overrides" &&
    input !== "delete_message" &&
    input !== "ban_member" &&
    input !== "view_audit_log" &&
    input !== "manage_ip_bans" &&
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

export function profileAboutFromInput(input: string): string {
  if (input.length > PROFILE_ABOUT_MAX_CHARS) {
    throw new DomainValidationError(`About must be 0-${PROFILE_ABOUT_MAX_CHARS} characters.`);
  }
  if (input.includes("\0")) {
    throw new DomainValidationError("About contains invalid characters.");
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
  visibility: GuildVisibility;
}

export interface ChannelRecord {
  channelId: ChannelId;
  name: ChannelName;
  kind: ChannelKindName;
}

export interface ChannelPermissionSnapshot {
  role: RoleName;
  permissions: PermissionName[];
}

export function channelPermissionSnapshotFromResponse(dto: unknown): ChannelPermissionSnapshot {
  const data = requireObject(dto, "channel permissions");
  const permissions = requireStringArray(data.permissions, "permissions", 64);
  return {
    role: roleFromInput(requireString(data.role, "role", 16)),
    permissions: permissions.map((entry) => permissionFromInput(entry)),
  };
}

export interface MessageRecord {
  messageId: MessageId;
  guildId: GuildId;
  channelId: ChannelId;
  authorId: UserId;
  content: MessageContent;
  markdownTokens: MarkdownToken[];
  attachments: AttachmentRecord[];
  reactions: ReactionRecord[];
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
  if (type === "heading_start") {
    const levelRaw = data.level;
    if (
      typeof levelRaw !== "number" ||
      !Number.isInteger(levelRaw) ||
      levelRaw < 1 ||
      levelRaw > 6
    ) {
      throw new DomainValidationError("Unsupported heading level.");
    }
    return {
      type,
      level: levelRaw,
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
  if (type === "fenced_code") {
    const languageRaw = data.language;
    let language: string | null = null;
    if (typeof languageRaw === "string") {
      if (
        languageRaw.length < 1 ||
        languageRaw.length > MAX_MARKDOWN_CODE_LANGUAGE_CHARS ||
        !/^[A-Za-z0-9_.+-]+$/.test(languageRaw)
      ) {
        throw new DomainValidationError("Unsupported fenced_code language.");
      }
      language = languageRaw.toLowerCase();
    } else if (languageRaw !== null && typeof languageRaw !== "undefined") {
      throw new DomainValidationError("Unsupported fenced_code language.");
    }
    return {
      type,
      language,
      code: requireString(data.code, "code", MAX_MARKDOWN_CODE_BLOCK_CHARS),
    };
  }
  if (
    type === "paragraph_start" ||
    type === "paragraph_end" ||
    type === "heading_end" ||
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
  const tokens = dto.map((entry) => markdownTokenFromResponse(entry));
  const fencedCodeCount = tokens.filter((token) => token.type === "fenced_code").length;
  if (fencedCodeCount > MAX_MARKDOWN_CODE_BLOCKS) {
    throw new DomainValidationError("markdown_tokens exceeded fenced code block limit.");
  }
  return tokens;
}

export function guildFromResponse(dto: unknown): GuildRecord {
  const data = requireObject(dto, "guild");
  return {
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    name: guildNameFromInput(requireString(data.name, "name")),
    visibility: guildVisibilityFromInput(requireString(data.visibility, "visibility", 16)),
  };
}

export function channelFromResponse(dto: unknown): ChannelRecord {
  const data = requireObject(dto, "channel");
  return {
    channelId: channelIdFromInput(requireString(data.channel_id, "channel_id")),
    name: channelNameFromInput(requireString(data.name, "name")),
    kind: channelKindFromInput(requireString(data.kind, "kind", 16)),
  };
}

export interface PublicGuildDirectory {
  guilds: GuildRecord[];
}

export function publicGuildDirectoryFromResponse(dto: unknown): PublicGuildDirectory {
  const data = requireObject(dto, "public guild directory");
  if (!Array.isArray(data.guilds)) {
    throw new DomainValidationError("Public guild directory must contain a guilds array.");
  }
  return {
    guilds: data.guilds.map((entry) => guildFromResponse(entry)),
  };
}

export type DirectoryJoinOutcome =
  | "accepted"
  | "already_member"
  | "rejected_visibility"
  | "rejected_user_ban"
  | "rejected_ip_ban";

export type DirectoryJoinErrorCode =
  | "directory_join_not_allowed"
  | "directory_join_user_banned"
  | "directory_join_ip_banned"
  | "rate_limited"
  | "forbidden"
  | "not_found"
  | "unexpected_error";

export interface DirectoryJoinResult {
  guildId: GuildId;
  outcome: DirectoryJoinOutcome;
  joined: boolean;
}

function directoryJoinOutcomeFromInput(input: string): DirectoryJoinOutcome {
  if (
    input !== "accepted" &&
    input !== "already_member" &&
    input !== "rejected_visibility" &&
    input !== "rejected_user_ban" &&
    input !== "rejected_ip_ban"
  ) {
    throw new DomainValidationError("Invalid directory join outcome.");
  }
  return input;
}

export function directoryJoinErrorCodeFromInput(input: string): DirectoryJoinErrorCode {
  if (
    input === "directory_join_not_allowed" ||
    input === "directory_join_user_banned" ||
    input === "directory_join_ip_banned" ||
    input === "rate_limited" ||
    input === "forbidden" ||
    input === "not_found" ||
    input === "unexpected_error"
  ) {
    return input;
  }
  return "unexpected_error";
}

export function directoryJoinResultFromResponse(dto: unknown): DirectoryJoinResult {
  const data = requireObject(dto, "directory join result");
  const outcome = directoryJoinOutcomeFromInput(requireString(data.outcome, "outcome", 64));
  return {
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    outcome,
    joined: outcome === "accepted" || outcome === "already_member",
  };
}

export interface GuildAuditEventRecord {
  auditId: string;
  actorUserId: UserId;
  targetUserId: UserId | null;
  action: string;
  createdAtUnix: number;
  ipBanMatch: boolean;
}

export interface GuildAuditPage {
  events: GuildAuditEventRecord[];
  nextCursor: AuditCursor | null;
}

function guildAuditEventFromResponse(dto: unknown): GuildAuditEventRecord {
  const data = requireObject(dto, "guild audit event");
  if (typeof data.details === "object" && data.details !== null) {
    if (hasRawIpField(data.details as Record<string, unknown>)) {
      throw new DomainValidationError("Guild audit event details must not expose raw IP fields.");
    }
  }
  return {
    auditId: requireString(data.audit_id, "audit_id", 26),
    actorUserId: userIdFromInput(requireString(data.actor_user_id, "actor_user_id")),
    targetUserId:
      data.target_user_id === null || typeof data.target_user_id === "undefined"
        ? null
        : userIdFromInput(requireString(data.target_user_id, "target_user_id")),
    action: requireString(data.action, "action", MAX_AUDIT_EVENT_ACTION_CHARS),
    createdAtUnix: requirePositiveInteger(data.created_at_unix, "created_at_unix"),
    ipBanMatch:
      typeof data.ip_ban_match === "boolean" ? requireBoolean(data.ip_ban_match, "ip_ban_match") : false,
  };
}

export function guildAuditPageFromResponse(dto: unknown): GuildAuditPage {
  const data = requireObject(dto, "guild audit page");
  if (!Array.isArray(data.events) || data.events.length > MAX_AUDIT_EVENTS_PER_PAGE) {
    throw new DomainValidationError("Guild audit page events must be a bounded array.");
  }
  const nextCursorRaw = data.next_cursor;
  return {
    events: data.events.map((entry) => guildAuditEventFromResponse(entry)),
    nextCursor:
      nextCursorRaw === null || typeof nextCursorRaw === "undefined"
        ? null
        : auditCursorFromInput(requireString(nextCursorRaw, "next_cursor", MAX_AUDIT_CURSOR_CHARS)),
  };
}

export interface GuildIpBanRecord {
  banId: GuildIpBanId;
  sourceUserId: UserId | null;
  reason: string | null;
  createdAtUnix: number;
  expiresAtUnix: number | null;
}

export interface GuildIpBanPage {
  bans: GuildIpBanRecord[];
  nextCursor: AuditCursor | null;
}

export interface GuildIpBanApplyResult {
  createdCount: number;
  banIds: GuildIpBanId[];
}

export interface GuildMemberRecord {
  userId: UserId;
  roleIds: WorkspaceRoleId[];
}

export interface GuildMemberPage {
  members: GuildMemberRecord[];
  nextCursor: UserId | null;
}

export interface GuildRoleRecord {
  roleId: WorkspaceRoleId;
  name: WorkspaceRoleName;
  position: number;
  isSystem: boolean;
  permissions: PermissionName[];
  colorHex?: RoleColorHex | null;
}

export interface GuildRoleList {
  roles: GuildRoleRecord[];
  defaultJoinRoleId?: WorkspaceRoleId | null;
}

function guildMemberRecordFromResponse(dto: unknown): GuildMemberRecord {
  const data = requireObject(dto, "guild member");
  const roleIdsDto = data.role_ids;
  const roleIdsRaw =
    typeof roleIdsDto === "undefined"
      ? []
      : requireStringArray(roleIdsDto, "role_ids", 64);
  if (roleIdsRaw.length > MAX_WORKSPACE_ROLE_ASSIGNMENTS_PER_USER) {
    throw new DomainValidationError("Guild member roles exceeds per-user cap.");
  }
  const deduped = new Set<WorkspaceRoleId>();
  for (const entry of roleIdsRaw) {
    deduped.add(workspaceRoleIdFromInput(entry));
  }
  return {
    userId: userIdFromInput(requireString(data.user_id, "user_id")),
    roleIds: [...deduped.values()],
  };
}

export function guildMemberPageFromResponse(dto: unknown): GuildMemberPage {
  const data = requireObject(dto, "guild member page");
  if (!Array.isArray(data.members) || data.members.length > MAX_GUILD_MEMBERS_PER_PAGE) {
    throw new DomainValidationError("Guild member page members must be a bounded array.");
  }
  const nextCursorRaw = data.next_cursor;
  return {
    members: data.members.map((entry) => guildMemberRecordFromResponse(entry)),
    nextCursor:
      nextCursorRaw === null || typeof nextCursorRaw === "undefined"
        ? null
        : userIdFromInput(requireString(nextCursorRaw, "next_cursor")),
  };
}

function guildIpBanRecordFromResponse(dto: unknown): GuildIpBanRecord {
  const data = requireObject(dto, "guild ip ban");
  if (hasRawIpField(data)) {
    throw new DomainValidationError("Guild IP ban payload must not include raw IP fields.");
  }
  const reasonRaw = data.reason;
  const reason =
    reasonRaw === null || typeof reasonRaw === "undefined"
      ? null
      : requireString(reasonRaw, "reason", MAX_GUILD_IP_BAN_REASON_CHARS);
  return {
    banId: guildIpBanIdFromInput(requireString(data.ban_id, "ban_id")),
    sourceUserId:
      data.source_user_id === null || typeof data.source_user_id === "undefined"
        ? null
        : userIdFromInput(requireString(data.source_user_id, "source_user_id")),
    reason,
    createdAtUnix: requirePositiveInteger(data.created_at_unix, "created_at_unix"),
    expiresAtUnix:
      data.expires_at_unix === null || typeof data.expires_at_unix === "undefined"
        ? null
        : requirePositiveInteger(data.expires_at_unix, "expires_at_unix"),
  };
}

export function guildIpBanPageFromResponse(dto: unknown): GuildIpBanPage {
  const data = requireObject(dto, "guild ip ban page");
  if (!Array.isArray(data.bans) || data.bans.length > MAX_GUILD_IP_BANS_PER_PAGE) {
    throw new DomainValidationError("Guild IP ban page bans must be a bounded array.");
  }
  const nextCursorRaw = data.next_cursor;
  return {
    bans: data.bans.map((entry) => guildIpBanRecordFromResponse(entry)),
    nextCursor:
      nextCursorRaw === null || typeof nextCursorRaw === "undefined"
        ? null
        : auditCursorFromInput(requireString(nextCursorRaw, "next_cursor", MAX_AUDIT_CURSOR_CHARS)),
  };
}

export function guildIpBanApplyResultFromResponse(dto: unknown): GuildIpBanApplyResult {
  const data = requireObject(dto, "guild ip ban apply result");
  if (!Array.isArray(data.ban_ids) || data.ban_ids.length > MAX_APPLIED_GUILD_IP_BAN_IDS) {
    throw new DomainValidationError("Guild IP ban apply result ban_ids must be a bounded array.");
  }
  return {
    createdCount: requireNonNegativeInteger(data.created_count, "created_count"),
    banIds: data.ban_ids.map((entry) => guildIpBanIdFromInput(requireString(entry, "ban_id"))),
  };
}

function guildRoleFromResponse(dto: unknown): GuildRoleRecord {
  const data = requireObject(dto, "guild role");
  const permissions = requireStringArray(
    data.permissions,
    "permissions",
    MAX_AUDIT_EVENT_ACTION_CHARS,
  );
  if (permissions.length > MAX_WORKSPACE_ROLE_PERMISSIONS) {
    throw new DomainValidationError("Guild role permissions exceeds per-role cap.");
  }
  const colorHexRaw = data.color_hex;
  return {
    roleId: workspaceRoleIdFromInput(requireString(data.role_id, "role_id")),
    name: workspaceRoleNameFromResponse(data.name),
    // System roles can legitimately use position 0 in server responses.
    position: requireNonNegativeInteger(data.position, "position"),
    isSystem: requireBoolean(data.is_system, "is_system"),
    permissions: permissions.map((entry) => permissionFromInput(entry)),
    colorHex:
      colorHexRaw === null || typeof colorHexRaw === "undefined"
        ? null
        : roleColorHexFromInput(requireString(colorHexRaw, "color_hex", 7)),
  };
}

export function guildRoleListFromResponse(dto: unknown): GuildRoleList {
  const data = requireObject(dto, "guild role list");
  if (!Array.isArray(data.roles) || data.roles.length > MAX_WORKSPACE_ROLES_PER_GUILD) {
    throw new DomainValidationError("Guild role list must be a bounded array.");
  }
  const defaultJoinRoleIdRaw = data.default_join_role_id;
  return {
    roles: data.roles.map((entry) => guildRoleFromResponse(entry)),
    defaultJoinRoleId:
      defaultJoinRoleIdRaw === null || typeof defaultJoinRoleIdRaw === "undefined"
        ? null
        : workspaceRoleIdFromInput(
            requireString(defaultJoinRoleIdRaw, "default_join_role_id"),
          ),
  };
}

export interface UserLookupRecord {
  userId: UserId;
  username: string;
  avatarVersion: number;
}

export function userLookupRecordFromResponse(dto: unknown): UserLookupRecord {
  const data = requireObject(dto, "user lookup record");
  const avatarVersionRaw = data.avatar_version;
  const avatarVersion =
    typeof avatarVersionRaw === "undefined"
      ? 0
      : requireNonNegativeInteger(avatarVersionRaw, "avatar_version");
  return {
    userId: userIdFromInput(requireString(data.user_id, "user_id")),
    username: usernameFromInput(requireString(data.username, "username", 64)),
    avatarVersion,
  };
}

export function userLookupListFromResponse(dto: unknown): UserLookupRecord[] {
  const data = requireObject(dto, "user lookup response");
  if (!Array.isArray(data.users)) {
    throw new DomainValidationError("User lookup response must include users array.");
  }
  if (data.users.length > 64) {
    throw new DomainValidationError("User lookup response exceeds maximum user records.");
  }
  return data.users.map((entry) => userLookupRecordFromResponse(entry));
}

export function messageFromResponse(dto: unknown): MessageRecord {
  const data = requireObject(dto, "message");
  const attachmentsDto = data.attachments;
  const attachments =
    typeof attachmentsDto === "undefined"
      ? []
      : Array.isArray(attachmentsDto)
        ? attachmentsDto.map((entry) => attachmentFromResponse(entry))
        : (() => {
          throw new DomainValidationError("attachments must be an array.");
        })();
  if (attachments.length > 5) {
    throw new DomainValidationError("attachments exceeds per-message cap.");
  }
  const reactionsDto = data.reactions;
  const reactions =
    typeof reactionsDto === "undefined"
      ? []
      : Array.isArray(reactionsDto)
        ? reactionsDto.map((entry) => reactionFromResponse(entry))
        : (() => {
          throw new DomainValidationError("reactions must be an array.");
        })();
  if (reactions.length > MAX_REACTIONS_PER_MESSAGE) {
    throw new DomainValidationError("reactions exceeds per-message cap.");
  }
  return {
    messageId: messageIdFromInput(requireString(data.message_id, "message_id")),
    guildId: guildIdFromInput(requireString(data.guild_id, "guild_id")),
    channelId: channelIdFromInput(requireString(data.channel_id, "channel_id")),
    authorId: userIdFromInput(requireString(data.author_id, "author_id")),
    content: messageContentFromInput(requireString(data.content, "content")),
    markdownTokens: markdownTokensFromResponse(data.markdown_tokens),
    attachments,
    reactions,
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
  reactedByMe: boolean | null;
  reactorUserIds: UserId[] | null;
}

export function reactionFromResponse(dto: unknown): ReactionRecord {
  const data = requireObject(dto, "reaction");
  const count = requireNonNegativeInteger(data.count, "count");
  const reactedByMeRaw = data.reacted_by_me;
  const reactedByMe =
    typeof reactedByMeRaw === "undefined"
      ? null
      : requireBoolean(reactedByMeRaw, "reacted_by_me");
  const reactorUserIdsRaw = data.reactor_user_ids;
  const reactorUserIds =
    typeof reactorUserIdsRaw === "undefined"
      ? null
      : (() => {
        if (!Array.isArray(reactorUserIdsRaw)) {
          throw new DomainValidationError("reactor_user_ids must be an array.");
        }
        if (reactorUserIdsRaw.length > MAX_REACTOR_USER_IDS_PER_REACTION) {
          throw new DomainValidationError("reactor_user_ids exceeds per-reaction cap.");
        }
        const parsed = reactorUserIdsRaw.map((entry, index) =>
          userIdFromInput(requireString(entry, `reactor_user_ids[${index}]`, 64)),
        );
        if (new Set(parsed).size !== parsed.length) {
          throw new DomainValidationError("reactor_user_ids contains duplicate entries.");
        }
        if (parsed.length > count) {
          throw new DomainValidationError("reactor_user_ids cannot exceed count.");
        }
        return parsed;
      })();
  if (reactedByMe === true && count === 0) {
    throw new DomainValidationError("reacted_by_me cannot be true when count is zero.");
  }
  return {
    emoji: reactionEmojiFromInput(requireString(data.emoji, "emoji")),
    count,
    reactedByMe,
    reactorUserIds,
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

export interface ProfileRecord {
  userId: UserId;
  username: string;
  aboutMarkdown: string;
  aboutMarkdownTokens: MarkdownToken[];
  avatarVersion: number;
  bannerVersion: number;
}

export function profileFromResponse(dto: unknown): ProfileRecord {
  const data = requireObject(dto, "profile");
  return {
    userId: userIdFromInput(requireString(data.user_id, "user_id")),
    username: usernameFromInput(requireString(data.username, "username", 64)),
    aboutMarkdown: profileAboutFromInput(
      requireString(data.about_markdown, "about_markdown", PROFILE_ABOUT_MAX_CHARS),
    ),
    aboutMarkdownTokens: markdownTokensFromResponse(data.about_markdown_tokens),
    avatarVersion: requireNonNegativeInteger(data.avatar_version, "avatar_version"),
    bannerVersion: requireNonNegativeInteger(data.banner_version, "banner_version"),
  };
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

export interface FriendRecord {
  userId: UserId;
  username: string;
  createdAtUnix: number;
}

export interface FriendRequestRecord {
  requestId: FriendRequestId;
  senderUserId: UserId;
  senderUsername: string;
  recipientUserId: UserId;
  recipientUsername: string;
  createdAtUnix: number;
}

export interface FriendRequestList {
  incoming: FriendRequestRecord[];
  outgoing: FriendRequestRecord[];
}

export interface FriendRequestCreateResult {
  requestId: FriendRequestId;
  senderUserId: UserId;
  recipientUserId: UserId;
  createdAtUnix: number;
}

function friendFromResponse(dto: unknown): FriendRecord {
  const data = requireObject(dto, "friend");
  return {
    userId: userIdFromInput(requireString(data.user_id, "user_id")),
    username: requireString(data.username, "username", 64),
    createdAtUnix: requireNonNegativeInteger(data.created_at_unix, "created_at_unix"),
  };
}

function friendRequestFromResponse(dto: unknown): FriendRequestRecord {
  const data = requireObject(dto, "friend request");
  return {
    requestId: friendRequestIdFromInput(requireString(data.request_id, "request_id")),
    senderUserId: userIdFromInput(requireString(data.sender_user_id, "sender_user_id")),
    senderUsername: requireString(data.sender_username, "sender_username", 64),
    recipientUserId: userIdFromInput(requireString(data.recipient_user_id, "recipient_user_id")),
    recipientUsername: requireString(data.recipient_username, "recipient_username", 64),
    createdAtUnix: requirePositiveInteger(data.created_at_unix, "created_at_unix"),
  };
}

export function friendListFromResponse(dto: unknown): FriendRecord[] {
  const data = requireObject(dto, "friend list");
  if (!Array.isArray(data.friends)) {
    throw new DomainValidationError("friends must be an array.");
  }
  return data.friends.map((entry) => friendFromResponse(entry));
}

export function friendRequestListFromResponse(dto: unknown): FriendRequestList {
  const data = requireObject(dto, "friend request list");
  if (!Array.isArray(data.incoming) || !Array.isArray(data.outgoing)) {
    throw new DomainValidationError("incoming and outgoing must be arrays.");
  }
  return {
    incoming: data.incoming.map((entry) => friendRequestFromResponse(entry)),
    outgoing: data.outgoing.map((entry) => friendRequestFromResponse(entry)),
  };
}

export function friendRequestCreateFromResponse(dto: unknown): FriendRequestCreateResult {
  const data = requireObject(dto, "friend request create");
  return {
    requestId: friendRequestIdFromInput(requireString(data.request_id, "request_id")),
    senderUserId: userIdFromInput(requireString(data.sender_user_id, "sender_user_id")),
    recipientUserId: userIdFromInput(requireString(data.recipient_user_id, "recipient_user_id")),
    createdAtUnix: requirePositiveInteger(data.created_at_unix, "created_at_unix"),
  };
}

export interface WorkspaceRecord {
  guildId: GuildId;
  guildName: GuildName;
  visibility: GuildVisibility;
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
    visibility:
      typeof data.visibility === "string"
        ? guildVisibilityFromInput(requireString(data.visibility, "visibility", 16))
        : "private",
    channels: channelsDto.map((channel) => {
      const channelObj = requireObject(channel, "channel cache");
      const kindValue = channelObj.kind;
      return {
        channelId: channelIdFromInput(requireString(channelObj.channelId, "channelId")),
        name: channelNameFromInput(requireString(channelObj.name, "name")),
        kind:
          typeof kindValue === "string"
            ? channelKindFromInput(requireString(kindValue, "kind", 16))
            : "text",
      };
    }),
  };
}
