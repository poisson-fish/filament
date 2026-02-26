import { DomainValidationError } from "../../domain/auth";
import {
  permissionFromInput,
  userIdFromInput,
  type ChannelId,
  type ChannelKindName,
  type GuildId,
  type MarkdownToken,
  type MessageId,
  type MessageRecord,
  type PermissionName,
  type ReactionEmoji,
  type RoleName,
  type UserId,
  type WorkspaceRecord,
} from "../../domain/chat";
import { ApiError } from "../../lib/api";
import { microphoneToggleErrorMessage } from "../../lib/browser-context";
import { RtcClientError, type RtcSnapshot } from "../../lib/rtc";

export interface ReactionView {
  count: number;
  reacted: boolean;
}

export interface MessageReactionView extends ReactionView {
  key: string;
  emoji: ReactionEmoji;
  pending: boolean;
}

export type MediaKind = "image" | "video" | "file";

export interface MessageMediaPreview {
  url: string;
  kind: MediaKind;
  mimeType: string;
}

const IMAGE_MIME_BY_EXTENSION: Record<string, string> = {
  avif: "image/avif",
  bmp: "image/bmp",
  gif: "image/gif",
  heic: "image/heic",
  heif: "image/heif",
  jpeg: "image/jpeg",
  jpg: "image/jpeg",
  png: "image/png",
  webp: "image/webp",
};

const VIDEO_MIME_BY_EXTENSION: Record<string, string> = {
  mov: "video/quicktime",
  mp4: "video/mp4",
  m4v: "video/mp4",
  webm: "video/webm",
};

export function reactionKey(messageId: MessageId, emoji: ReactionEmoji): string {
  return `${messageId}|${emoji}`;
}

function reactionPrefix(messageId: MessageId): string {
  return `${messageId}|`;
}

export function upsertReactionEntry(
  existing: Record<string, ReactionView>,
  key: string,
  nextReaction: ReactionView,
): Record<string, ReactionView> {
  const next = { ...existing };
  if (nextReaction.count <= 0 && !nextReaction.reacted) {
    delete next[key];
  } else {
    next[key] = nextReaction;
  }
  return next;
}

export function clearKeysByPrefix<T>(existing: Record<string, T>, prefix: string): Record<string, T> {
  const next = { ...existing };
  let changed = false;
  for (const key of Object.keys(next)) {
    if (!key.startsWith(prefix)) {
      continue;
    }
    delete next[key];
    changed = true;
  }
  return changed ? next : existing;
}

export function reactionViewsForMessage(
  messageId: MessageId,
  reactions: Record<string, ReactionView>,
  pendingByKey: Record<string, true>,
): MessageReactionView[] {
  const prefix = reactionPrefix(messageId);
  const list: MessageReactionView[] = [];
  for (const [key, state] of Object.entries(reactions)) {
    if (!key.startsWith(prefix)) {
      continue;
    }
    if (state.count < 1 && !state.reacted) {
      continue;
    }
    list.push({
      key,
      emoji: key.slice(prefix.length) as ReactionEmoji,
      count: state.count,
      reacted: state.reacted,
      pending: Boolean(pendingByKey[key]),
    });
  }
  list.sort((left, right) => {
    if (left.count !== right.count) {
      return right.count - left.count;
    }
    return left.emoji.localeCompare(right.emoji);
  });
  return list;
}

export function channelKey(guildId: GuildId, channelId: ChannelId): string {
  return `${guildId}|${channelId}`;
}

export function parseChannelKey(value: string): { guildId: GuildId; channelId: ChannelId } | null {
  const [guildId, channelId, ...rest] = value.split("|");
  if (!guildId || !channelId || rest.length > 0) {
    return null;
  }
  return {
    guildId: guildId as GuildId,
    channelId: channelId as ChannelId,
  };
}

export function formatVoiceDuration(totalSeconds: number): string {
  const safeSeconds = Number.isFinite(totalSeconds) ? Math.max(0, Math.floor(totalSeconds)) : 0;
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const seconds = safeSeconds % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  const kib = value / 1024;
  if (kib < 1024) {
    return `${kib.toFixed(1)} KiB`;
  }
  return `${(kib / 1024).toFixed(2)} MiB`;
}

function classifyAttachmentMediaType(mimeType: string): MediaKind {
  const lower = mimeType.toLowerCase();
  if (lower.startsWith("image/") && lower !== "image/svg+xml") {
    return "image";
  }
  if (lower.startsWith("video/")) {
    return "video";
  }
  return "file";
}

export function createObjectUrl(blob: Blob): string | null {
  if (typeof URL.createObjectURL !== "function") {
    return null;
  }
  return URL.createObjectURL(blob);
}

export function revokeObjectUrl(url: string): void {
  if (typeof URL.revokeObjectURL !== "function") {
    return;
  }
  URL.revokeObjectURL(url);
}

function extensionFromFilename(filename: string): string | null {
  const dot = filename.lastIndexOf(".");
  if (dot <= 0 || dot === filename.length - 1) {
    return null;
  }
  return filename.slice(dot + 1).toLowerCase();
}

function inferMimeTypeFromFilename(filename: string): string | null {
  const extension = extensionFromFilename(filename);
  if (!extension) {
    return null;
  }
  return IMAGE_MIME_BY_EXTENSION[extension] ?? VIDEO_MIME_BY_EXTENSION[extension] ?? null;
}

function normalizeMimeType(raw: string | null | undefined): string {
  if (!raw) {
    return "";
  }
  return raw.split(";")[0].trim().toLowerCase();
}

export function resolveAttachmentPreviewType(
  payloadMimeType: string | null,
  attachmentMimeType: string,
  filename: string,
): { kind: MediaKind; mimeType: string } {
  const normalizedPayloadMime = normalizeMimeType(payloadMimeType);
  const normalizedAttachmentMime = normalizeMimeType(attachmentMimeType);
  const inferredFromFilename = inferMimeTypeFromFilename(filename);
  const fallbackMime =
    (inferredFromFilename ?? normalizedAttachmentMime) || "application/octet-stream";
  const resolvedMimeType = normalizedPayloadMime || fallbackMime;
  let kind = classifyAttachmentMediaType(resolvedMimeType);
  if (kind === "file" && inferredFromFilename) {
    kind = classifyAttachmentMediaType(inferredFromFilename);
  }
  return {
    kind,
    mimeType: kind === "file" ? resolvedMimeType : inferredFromFilename ?? resolvedMimeType,
  };
}

export function mapError(error: unknown, fallback: string): string {
  if (error instanceof DomainValidationError) {
    return error.message;
  }
  if (error instanceof ApiError) {
    if (error.code === "rate_limited") {
      return "Rate limited. Please wait and retry.";
    }
    if (error.code === "forbidden") {
      return "Permission denied for this action.";
    }
    if (error.code === "not_found") {
      return "Requested resource was not found.";
    }
    if (error.code === "network_error") {
      return "Cannot reach server. Verify API origin and TLS setup.";
    }
    if (error.code === "request_timeout") {
      return "Server timed out while processing the request.";
    }
    if (error.code === "payload_too_large") {
      return "Payload is too large for this endpoint.";
    }
    if (error.code === "quota_exceeded") {
      return "Attachment quota exceeded for this user.";
    }
    if (error.code === "guild_creation_limit_reached") {
      return "Guild creation limit reached for this account.";
    }
    if (error.code === "invalid_credentials") {
      return "Authentication failed. Please login again.";
    }
    if (error.code === "invalid_request") {
      return "Request payload did not pass API validation.";
    }
    if (error.code === "protocol_mismatch") {
      return "Server/client protocol mismatch for attachments. Rebuild and restart filament-server.";
    }
    if (error.code === "internal_error") {
      return "Server reported an internal error. Retry in a moment.";
    }
    return `Request failed (${error.code}).`;
  }
  return fallback;
}

export function mapRtcError(error: unknown, fallback: string): string {
  if (error instanceof RtcClientError) {
    if (error.code === "invalid_livekit_url") {
      return "Voice signaling URL is invalid for this environment.";
    }
    if (error.code === "invalid_livekit_token") {
      return "Voice token was rejected by local validation.";
    }
    if (error.code === "invalid_audio_device_id") {
      return "Selected audio device is invalid.";
    }
    if (error.code === "join_failed") {
      return "Unable to connect to voice right now.";
    }
    if (error.code === "not_connected") {
      return "Voice is not connected.";
    }
    if (error.code === "microphone_toggle_failed") {
      return microphoneToggleErrorMessage(error.message);
    }
    if (error.code === "camera_toggle_failed") {
      return "Unable to change camera state.";
    }
    if (error.code === "screen_share_toggle_failed") {
      return "Unable to change screen share state.";
    }
    if (error.code === "audio_device_switch_failed") {
      return "Unable to switch the selected audio device.";
    }
    return error.message;
  }
  return fallback;
}

function isLikelyTokenExpiryMessage(message: string): boolean {
  const normalized = message.toLowerCase();
  return normalized.includes("token") && normalized.includes("expir");
}

export function mapVoiceJoinError(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.code === "invalid_credentials") {
      return "Voice token request expired with your session. Refresh session or login again, then retry Join Voice.";
    }
    if (error.code === "forbidden") {
      return "Voice join rejected by channel permissions or overrides.";
    }
    if (error.code === "rate_limited") {
      return "Voice token request is rate limited. Wait a moment and retry.";
    }
    return mapError(error, "Unable to join voice.");
  }
  if (error instanceof RtcClientError && error.code === "join_failed") {
    if (isLikelyTokenExpiryMessage(error.message)) {
      return "Voice token expired before signaling completed. Select Join Voice to request a fresh token.";
    }
    return "Voice connection failed. Verify LiveKit signaling reachability and retry.";
  }
  return mapRtcError(error, "Unable to join voice.");
}

export function voiceConnectionLabel(snapshot: RtcSnapshot): string {
  if (snapshot.connectionStatus === "connecting") {
    return "connecting";
  }
  if (snapshot.connectionStatus === "connected") {
    return "connected";
  }
  if (snapshot.connectionStatus === "reconnecting") {
    return "reconnecting";
  }
  if (snapshot.connectionStatus === "error") {
    return "error";
  }
  return "disconnected";
}

export function profileErrorMessage(error: unknown): string {
  if (error instanceof ApiError && error.code === "invalid_credentials") {
    return "Session expired. Please login again.";
  }
  return mapError(error, "Profile unavailable.");
}

export function formatMessageTime(createdAtUnix: number): string {
  return new Date(createdAtUnix * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function shortActor(value: string): string {
  return value.length > 14 ? `${value.slice(0, 14)}...` : value;
}

export function userIdFromVoiceIdentity(identity: string): UserId | null {
  const [prefix, rawUserId, ...rest] = identity.split(".");
  if (prefix !== "u" || !rawUserId || rest.length === 0) {
    return null;
  }
  try {
    return userIdFromInput(rawUserId);
  } catch {
    return null;
  }
}

export function actorAvatarGlyph(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return "?";
  }
  const [first, second] = trimmed.split(/\s+/);
  if (second && first.length > 0) {
    return `${first[0]!}${second[0]!}`.toUpperCase();
  }
  return trimmed.slice(0, 1).toUpperCase();
}

export function upsertWorkspace(
  existing: WorkspaceRecord[],
  guildId: GuildId,
  updater: (workspace: WorkspaceRecord) => WorkspaceRecord,
): WorkspaceRecord[] {
  return existing.map((workspace) => (workspace.guildId === guildId ? updater(workspace) : workspace));
}

function compareMessageChronology(left: MessageRecord, right: MessageRecord): number {
  if (left.createdAtUnix !== right.createdAtUnix) {
    return left.createdAtUnix - right.createdAtUnix;
  }
  if (left.messageId === right.messageId) {
    return 0;
  }
  return left.messageId < right.messageId ? -1 : 1;
}

export function normalizeMessageOrder(messages: MessageRecord[]): MessageRecord[] {
  return [...messages].sort(compareMessageChronology);
}

export function mergeMessage(existing: MessageRecord[], incoming: MessageRecord): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === incoming.messageId);
  const next = [...existing];
  if (index >= 0) {
    next[index] = incoming;
    return normalizeMessageOrder(next);
  }
  next.push(incoming);
  return normalizeMessageOrder(next);
}

export function mergeMessageHistory(
  existing: MessageRecord[],
  incoming: MessageRecord[],
): MessageRecord[] {
  const byId = new Map(existing.map((entry) => [entry.messageId, entry]));
  for (const entry of incoming) {
    if (!byId.has(entry.messageId)) {
      byId.set(entry.messageId, entry);
    }
  }
  return normalizeMessageOrder([...byId.values()]);
}

export function tokenizeToDisplayText(tokens: MarkdownToken[]): string {
  let output = "";
  let pendingLink: string | null = null;

  for (const token of tokens) {
    if (token.type === "text") {
      output += token.text;
      continue;
    }
    if (token.type === "code") {
      output += `\`${token.code}\``;
      continue;
    }
    if (token.type === "fenced_code") {
      const languageLabel = token.language ? token.language : "";
      output += `\n\`\`\`${languageLabel}\n${token.code}\n\`\`\`\n`;
      continue;
    }
    if (token.type === "soft_break" || token.type === "hard_break") {
      output += "\n";
      continue;
    }
    if (token.type === "paragraph_end") {
      output += "\n\n";
      continue;
    }
    if (token.type === "list_item_start") {
      output += "• ";
      continue;
    }
    if (token.type === "list_item_end") {
      output += "\n";
      continue;
    }
    if (token.type === "link_start") {
      pendingLink = token.href;
      continue;
    }
    if (token.type === "link_end") {
      if (pendingLink) {
        output += ` (${pendingLink})`;
      }
      pendingLink = null;
    }
  }

  return output.trimEnd();
}

export function parsePermissionCsv(value: string): PermissionName[] {
  const unique = new Set<PermissionName>();
  const tokens = value
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
  for (const token of tokens) {
    unique.add(permissionFromInput(token));
  }
  return [...unique];
}

export function canDiscoverWorkspaceOperation(role: RoleName | undefined): boolean {
  return role === "owner" || role === "moderator";
}

export function channelRailLabel(input: { kind: ChannelKindName; name: string }): string {
  if (input.kind === "voice") {
    return input.name;
  }
  return `#${input.name}`;
}

export function channelHeaderLabel(input: { kind: ChannelKindName; name: string }): string {
  if (input.kind === "voice") {
    return `Voice: ${input.name}`;
  }
  return `#${input.name}`;
}
