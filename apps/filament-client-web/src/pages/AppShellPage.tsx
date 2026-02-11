import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
  untrack,
} from "solid-js";
import { DomainValidationError } from "../domain/auth";
import {
  attachmentFilenameFromInput,
  channelKindFromInput,
  channelNameFromInput,
  guildVisibilityFromInput,
  guildNameFromInput,
  messageContentFromInput,
  permissionFromInput,
  reactionEmojiFromInput,
  roleFromInput,
  searchQueryFromInput,
  userIdFromInput,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelId,
  type ChannelKindName,
  type ChannelPermissionSnapshot,
  type FriendRecord,
  type FriendRequestList,
  type GuildVisibility,
  type GuildId,
  type MarkdownToken,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
  type GuildRecord,
  type MediaPublishSource,
  type PermissionName,
  type RoleName,
  type SearchResults,
  type UserId,
  type WorkspaceRecord,
} from "../domain/chat";
import {
  ApiError,
  addGuildMember,
  acceptFriendRequest,
  addMessageReaction,
  banGuildMember,
  createChannel,
  createChannelMessage,
  createFriendRequest,
  createGuild,
  deleteFriendRequest,
  deleteChannelAttachment,
  deleteChannelMessage,
  downloadChannelAttachmentPreview,
  downloadChannelAttachment,
  editChannelMessage,
  echoMessage,
  fetchChannelMessages,
  fetchChannelPermissionSnapshot,
  fetchGuildChannels,
  fetchGuilds,
  fetchFriendRequests,
  fetchFriends,
  fetchHealth,
  fetchMe,
  fetchPublicGuildDirectory,
  issueVoiceToken,
  kickGuildMember,
  logoutAuthSession,
  rebuildGuildSearchIndex,
  reconcileGuildSearchIndex,
  refreshAuthSession,
  removeMessageReaction,
  removeFriend,
  searchGuildMessages,
  setChannelRoleOverride,
  updateGuildMemberRole,
  uploadChannelAttachment,
} from "../lib/api";
import { useAuth } from "../lib/auth-context";
import { connectGateway } from "../lib/gateway";
import {
  RtcClientError,
  createRtcClient,
  type RtcClient,
  type RtcSnapshot,
} from "../lib/rtc";
import {
  enumerateAudioDevices,
  loadVoiceDevicePreferences,
  reconcileVoiceDevicePreferences,
  saveVoiceDevicePreferences,
  type AudioDeviceOption,
  type MediaDeviceId,
  type VoiceDevicePreferences,
} from "../lib/voice-device-settings";
import { clearWorkspaceCache, saveWorkspaceCache } from "../lib/workspace-cache";
import {
  clearUsernameLookupCache,
  primeUsernameCache,
  resolveUsernames,
} from "../lib/username-cache";

const ADD_REACTION_ICON_URL = new URL(
  "../../resource/coolicons.v4.1/cooliocns SVG/Edit/Add_Plus_Circle.svg",
  import.meta.url,
).href;
const EDIT_MESSAGE_ICON_URL = new URL(
  "../../resource/coolicons.v4.1/cooliocns SVG/Edit/Edit_Pencil_Line_01.svg",
  import.meta.url,
).href;
const DELETE_MESSAGE_ICON_URL = new URL(
  "../../resource/coolicons.v4.1/cooliocns SVG/Interface/Trash_Full.svg",
  import.meta.url,
).href;
const MAX_COMPOSER_ATTACHMENTS = 5;
const MAX_EMBED_PREVIEW_BYTES = 25 * 1024 * 1024;
const MAX_MEDIA_PREVIEW_RETRIES = 2;
const INITIAL_MEDIA_PREVIEW_DELAY_MS = 75;
const RTC_DISCONNECTED_SNAPSHOT: RtcSnapshot = {
  connectionStatus: "disconnected",
  localParticipantIdentity: null,
  isMicrophoneEnabled: false,
  isCameraEnabled: false,
  isScreenShareEnabled: false,
  participants: [],
  videoTracks: [],
  activeSpeakerIdentities: [],
  lastErrorCode: null,
  lastErrorMessage: null,
};
const DEFAULT_VOICE_SESSION_CAPABILITIES: VoiceSessionCapabilities = {
  canSubscribe: false,
  publishSources: [],
};

interface ReactionView {
  count: number;
  reacted: boolean;
}

interface MessageReactionView extends ReactionView {
  key: string;
  emoji: ReactionEmoji;
  pending: boolean;
}

interface VoiceRosterEntry {
  identity: string;
  isLocal: boolean;
  isSpeaking: boolean;
  hasCamera: boolean;
  hasScreenShare: boolean;
}

interface VoiceSessionCapabilities {
  canSubscribe: boolean;
  publishSources: MediaPublishSource[];
}

interface ReactionPickerOption {
  emoji: ReactionEmoji;
  label: string;
  iconUrl: string;
}

const OPENMOJI_REACTION_OPTIONS: ReactionPickerOption[] = [
  {
    emoji: reactionEmojiFromInput("👍"),
    label: "Thumbs up",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F44D.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👎"),
    label: "Thumbs down",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F44E.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😂"),
    label: "Tears of joy",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F602.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤣"),
    label: "Rolling on the floor laughing",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F923.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😮"),
    label: "Surprised",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F62E.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😢"),
    label: "Crying",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F622.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("😱"),
    label: "Screaming",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F631.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👏"),
    label: "Clapping",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F44F.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🔥"),
    label: "Fire",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F525.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🎉"),
    label: "Party popper",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F389.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤔"),
    label: "Thinking",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F914.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🙌"),
    label: "Raised hands",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F64C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🚀"),
    label: "Rocket",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F680.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💯"),
    label: "Hundred points",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F4AF.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🏆"),
    label: "Trophy",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F3C6.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🤝"),
    label: "Handshake",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F91D.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🙏"),
    label: "Folded hands",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F64F.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("👌"),
    label: "Ok hand",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F44C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("✅"),
    label: "Check mark",
    iconUrl: new URL("../../resource/openmoji-svg-color/2705.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("❌"),
    label: "Cross mark",
    iconUrl: new URL("../../resource/openmoji-svg-color/274C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("❤"),
    label: "Heart",
    iconUrl: new URL("../../resource/openmoji-svg-color/2764.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💜"),
    label: "Purple heart",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F49C.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("🧠"),
    label: "Brain",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F9E0.svg", import.meta.url).href,
  },
  {
    emoji: reactionEmojiFromInput("💡"),
    label: "Light bulb",
    iconUrl: new URL("../../resource/openmoji-svg-color/1F4A1.svg", import.meta.url).href,
  },
];

type MediaKind = "image" | "video" | "file";

interface MessageMediaPreview {
  url: string;
  kind: MediaKind;
  mimeType: string;
}

function reactionKey(messageId: MessageId, emoji: ReactionEmoji): string {
  return `${messageId}|${emoji}`;
}

function reactionPrefix(messageId: MessageId): string {
  return `${messageId}|`;
}

function upsertReactionEntry(
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

function clearKeysByPrefix<T>(existing: Record<string, T>, prefix: string): Record<string, T> {
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

function reactionViewsForMessage(
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

function channelKey(guildId: GuildId, channelId: ChannelId): string {
  return `${guildId}|${channelId}`;
}

function parseChannelKey(value: string): { guildId: GuildId; channelId: ChannelId } | null {
  const [guildId, channelId, ...rest] = value.split("|");
  if (!guildId || !channelId || rest.length > 0) {
    return null;
  }
  return {
    guildId: guildId as GuildId,
    channelId: channelId as ChannelId,
  };
}

function formatVoiceDuration(totalSeconds: number): string {
  const safeSeconds = Number.isFinite(totalSeconds) ? Math.max(0, Math.floor(totalSeconds)) : 0;
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const seconds = safeSeconds % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function formatBytes(value: number): string {
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

function createObjectUrl(blob: Blob): string | null {
  if (typeof URL.createObjectURL !== "function") {
    return null;
  }
  return URL.createObjectURL(blob);
}

function revokeObjectUrl(url: string): void {
  if (typeof URL.revokeObjectURL !== "function") {
    return;
  }
  URL.revokeObjectURL(url);
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

function resolveAttachmentPreviewType(
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

function mapError(error: unknown, fallback: string): string {
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
    return `Request failed (${error.code}).`;
  }
  return fallback;
}

function mapRtcError(error: unknown, fallback: string): string {
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
      return "Unable to change microphone state.";
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

function mapVoiceJoinError(error: unknown): string {
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

function voiceConnectionLabel(snapshot: RtcSnapshot): string {
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

function profileErrorMessage(error: unknown): string {
  if (error instanceof ApiError && error.code === "invalid_credentials") {
    return "Session expired. Please login again.";
  }
  return mapError(error, "Profile unavailable.");
}

function formatMessageTime(createdAtUnix: number): string {
  return new Date(createdAtUnix * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function shortActor(value: string): string {
  return value.length > 14 ? `${value.slice(0, 14)}...` : value;
}

function actorAvatarGlyph(value: string): string {
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

function upsertWorkspace(
  existing: WorkspaceRecord[],
  guildId: GuildId,
  updater: (workspace: WorkspaceRecord) => WorkspaceRecord,
): WorkspaceRecord[] {
  return existing.map((workspace) => (workspace.guildId === guildId ? updater(workspace) : workspace));
}

function mergeMessage(existing: MessageRecord[], incoming: MessageRecord): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === incoming.messageId);
  if (index >= 0) {
    const next = [...existing];
    next[index] = incoming;
    return next;
  }
  return [...existing, incoming];
}

function prependOlderMessages(
  existing: MessageRecord[],
  olderAscending: MessageRecord[],
): MessageRecord[] {
  const known = new Set(existing.map((entry) => entry.messageId));
  const prepend = olderAscending.filter((entry) => !known.has(entry.messageId));
  return [...prepend, ...existing];
}

function tokenizeToDisplayText(tokens: MarkdownToken[]): string {
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

function parsePermissionCsv(value: string): PermissionName[] {
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

function canDiscoverWorkspaceOperation(
  role: RoleName | undefined,
): boolean {
  return role === "owner" || role === "moderator";
}

function channelRailLabel(input: { kind: ChannelKindName; name: string }): string {
  if (input.kind === "voice") {
    return input.name;
  }
  return `#${input.name}`;
}

function channelHeaderLabel(input: { kind: ChannelKindName; name: string }): string {
  if (input.kind === "voice") {
    return `Voice: ${input.name}`;
  }
  return `#${input.name}`;
}

type OverlayPanel =
  | "workspace-create"
  | "channel-create"
  | "settings"
  | "public-directory"
  | "friendships"
  | "search"
  | "attachments"
  | "moderation"
  | "utility";

type SettingsCategory = "voice" | "profile";
type VoiceSettingsSubmenu = "audio-devices";

interface SettingsCategoryItem {
  id: SettingsCategory;
  label: string;
  summary: string;
}

interface VoiceSettingsSubmenuItem {
  id: VoiceSettingsSubmenu;
  label: string;
  summary: string;
}

const SETTINGS_CATEGORIES: SettingsCategoryItem[] = [
  {
    id: "voice",
    label: "Voice",
    summary: "Audio devices and call behavior.",
  },
  {
    id: "profile",
    label: "Profile",
    summary: "Account and identity placeholder.",
  },
];

const VOICE_SETTINGS_SUBMENU: VoiceSettingsSubmenuItem[] = [
  {
    id: "audio-devices",
    label: "Audio Devices",
    summary: "Select microphone and speaker devices.",
  },
];

export function AppShellPage() {
  const auth = useAuth();
  let composerAttachmentInputRef: HTMLInputElement | undefined;
  const inflightMessageMediaLoads = new Set<string>();
  const previewRetryAttempts = new Map<string, number>();
  let previewSessionRefreshPromise: Promise<void> | null = null;

  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
  const [activeGuildId, setActiveGuildId] = createSignal<GuildId | null>(null);
  const [activeChannelId, setActiveChannelId] = createSignal<ChannelId | null>(null);
  const [workspaceBootstrapDone, setWorkspaceBootstrapDone] = createSignal(false);

  const [composer, setComposer] = createSignal("");
  const [messageStatus, setMessageStatus] = createSignal("");
  const [messageError, setMessageError] = createSignal("");
  const [isLoadingMessages, setLoadingMessages] = createSignal(false);
  const [isLoadingOlder, setLoadingOlder] = createSignal(false);
  const [isSendingMessage, setSendingMessage] = createSignal(false);
  const [messages, setMessages] = createSignal<MessageRecord[]>([]);
  const [messageMediaByAttachmentId, setMessageMediaByAttachmentId] = createSignal<
    Record<string, MessageMediaPreview>
  >({});
  const [loadingMediaPreviewIds, setLoadingMediaPreviewIds] = createSignal<Record<string, true>>(
    {},
  );
  const [failedMediaPreviewIds, setFailedMediaPreviewIds] = createSignal<Record<string, true>>({});
  const [mediaPreviewRetryTick, setMediaPreviewRetryTick] = createSignal(0);
  const [nextBefore, setNextBefore] = createSignal<MessageId | null>(null);
  const [reactionState, setReactionState] = createSignal<Record<string, ReactionView>>({});
  const [pendingReactionByKey, setPendingReactionByKey] = createSignal<Record<string, true>>({});
  const [openReactionPickerMessageId, setOpenReactionPickerMessageId] = createSignal<MessageId | null>(null);
  const [editingMessageId, setEditingMessageId] = createSignal<MessageId | null>(null);
  const [editingDraft, setEditingDraft] = createSignal("");
  const [isSavingEdit, setSavingEdit] = createSignal(false);
  const [deletingMessageId, setDeletingMessageId] = createSignal<MessageId | null>(null);
  const [composerAttachments, setComposerAttachments] = createSignal<File[]>([]);

  const [createGuildName, setCreateGuildName] = createSignal("Security Ops");
  const [createGuildVisibility, setCreateGuildVisibility] = createSignal<GuildVisibility>("private");
  const [createChannelName, setCreateChannelName] = createSignal("incident-room");
  const [createChannelKind, setCreateChannelKind] = createSignal<ChannelKindName>("text");
  const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
  const [workspaceError, setWorkspaceError] = createSignal("");
  const [publicGuildSearchQuery, setPublicGuildSearchQuery] = createSignal("");
  const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
  const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
  const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<GuildRecord[]>([]);
  const [friendRecipientUserIdInput, setFriendRecipientUserIdInput] = createSignal("");
  const [friends, setFriends] = createSignal<FriendRecord[]>([]);
  const [friendRequests, setFriendRequests] = createSignal<FriendRequestList>({
    incoming: [],
    outgoing: [],
  });
  const [isRunningFriendAction, setRunningFriendAction] = createSignal(false);
  const [friendStatus, setFriendStatus] = createSignal("");
  const [friendError, setFriendError] = createSignal("");

  const [newChannelName, setNewChannelName] = createSignal("backend");
  const [newChannelKind, setNewChannelKind] = createSignal<ChannelKindName>("text");
  const [isCreatingChannel, setCreatingChannel] = createSignal(false);
  const [channelCreateError, setChannelCreateError] = createSignal("");

  const [searchQuery, setSearchQuery] = createSignal("");
  const [searchError, setSearchError] = createSignal("");
  const [isSearching, setSearching] = createSignal(false);
  const [searchResults, setSearchResults] = createSignal<SearchResults | null>(null);
  const [isRunningSearchOps, setRunningSearchOps] = createSignal(false);
  const [searchOpsStatus, setSearchOpsStatus] = createSignal("");

  const [gatewayOnline, setGatewayOnline] = createSignal(false);
  const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);
  const [resolvedUsernames, setResolvedUsernames] = createSignal<Record<string, string>>({});

  const [attachmentByChannel, setAttachmentByChannel] = createSignal<Record<string, AttachmentRecord[]>>({});
  const [selectedAttachment, setSelectedAttachment] = createSignal<File | null>(null);
  const [attachmentFilename, setAttachmentFilename] = createSignal("");
  const [attachmentStatus, setAttachmentStatus] = createSignal("");
  const [attachmentError, setAttachmentError] = createSignal("");
  const [isUploadingAttachment, setUploadingAttachment] = createSignal(false);
  const [downloadingAttachmentId, setDownloadingAttachmentId] = createSignal<AttachmentId | null>(null);
  const [deletingAttachmentId, setDeletingAttachmentId] = createSignal<AttachmentId | null>(null);

  const [rtcSnapshot, setRtcSnapshot] = createSignal<RtcSnapshot>(RTC_DISCONNECTED_SNAPSHOT);
  const [voiceStatus, setVoiceStatus] = createSignal("");
  const [voiceError, setVoiceError] = createSignal("");
  const [isJoiningVoice, setJoiningVoice] = createSignal(false);
  const [isLeavingVoice, setLeavingVoice] = createSignal(false);
  const [isTogglingVoiceMic, setTogglingVoiceMic] = createSignal(false);
  const [isTogglingVoiceCamera, setTogglingVoiceCamera] = createSignal(false);
  const [isTogglingVoiceScreenShare, setTogglingVoiceScreenShare] = createSignal(false);
  const [voiceSessionChannelKey, setVoiceSessionChannelKey] = createSignal<string | null>(null);
  const [voiceSessionStartedAtUnixMs, setVoiceSessionStartedAtUnixMs] = createSignal<number | null>(
    null,
  );
  const [voiceDurationClockUnixMs, setVoiceDurationClockUnixMs] = createSignal(Date.now());
  const [voiceSessionCapabilities, setVoiceSessionCapabilities] = createSignal<VoiceSessionCapabilities>(
    DEFAULT_VOICE_SESSION_CAPABILITIES,
  );

  const [moderationUserIdInput, setModerationUserIdInput] = createSignal("");
  const [moderationRoleInput, setModerationRoleInput] = createSignal<RoleName>("member");
  const [isModerating, setModerating] = createSignal(false);
  const [moderationStatus, setModerationStatus] = createSignal("");
  const [moderationError, setModerationError] = createSignal("");

  const [overrideRoleInput, setOverrideRoleInput] = createSignal<RoleName>("member");
  const [overrideAllowCsv, setOverrideAllowCsv] = createSignal("create_message");
  const [overrideDenyCsv, setOverrideDenyCsv] = createSignal("");

  const [isRefreshingSession, setRefreshingSession] = createSignal(false);
  const [sessionStatus, setSessionStatus] = createSignal("");
  const [sessionError, setSessionError] = createSignal("");
  const [channelPermissions, setChannelPermissions] = createSignal<ChannelPermissionSnapshot | null>(null);

  const [healthStatus, setHealthStatus] = createSignal("");
  const [echoInput, setEchoInput] = createSignal("hello filament");
  const [diagError, setDiagError] = createSignal("");
  const [isCheckingHealth, setCheckingHealth] = createSignal(false);
  const [isEchoing, setEchoing] = createSignal(false);
  const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(null);
  const [activeSettingsCategory, setActiveSettingsCategory] = createSignal<SettingsCategory>("voice");
  const [activeVoiceSettingsSubmenu, setActiveVoiceSettingsSubmenu] =
    createSignal<VoiceSettingsSubmenu>("audio-devices");
  const [voiceDevicePreferences, setVoiceDevicePreferences] = createSignal<VoiceDevicePreferences>(
    loadVoiceDevicePreferences(),
  );
  const [audioInputDevices, setAudioInputDevices] = createSignal<AudioDeviceOption[]>([]);
  const [audioOutputDevices, setAudioOutputDevices] = createSignal<AudioDeviceOption[]>([]);
  const [isRefreshingAudioDevices, setRefreshingAudioDevices] = createSignal(false);
  const [audioDevicesStatus, setAudioDevicesStatus] = createSignal("");
  const [audioDevicesError, setAudioDevicesError] = createSignal("");
  const [isChannelRailCollapsed, setChannelRailCollapsed] = createSignal(false);
  const [isMemberRailCollapsed, setMemberRailCollapsed] = createSignal(false);
  let previousVoiceConnectionStatus: RtcSnapshot["connectionStatus"] =
    RTC_DISCONNECTED_SNAPSHOT.connectionStatus;
  let rtcClient: RtcClient | null = null;
  let stopRtcSubscription: (() => void) | null = null;

  const activeWorkspace = createMemo(
    () => workspaces().find((workspace) => workspace.guildId === activeGuildId()) ?? null,
  );

  const activeChannel = createMemo(
    () =>
      activeWorkspace()?.channels.find((channel) => channel.channelId === activeChannelId()) ??
      null,
  );
  const activeTextChannels = createMemo(() =>
    (activeWorkspace()?.channels ?? []).filter((channel) => channel.kind === "text"),
  );
  const activeVoiceChannels = createMemo(() =>
    (activeWorkspace()?.channels ?? []).filter((channel) => channel.kind === "voice"),
  );
  const isActiveVoiceChannel = createMemo(() => activeChannel()?.kind === "voice");

  const hasPermission = (permission: PermissionName): boolean =>
    channelPermissions()?.permissions.includes(permission) ?? false;

  const canAccessActiveChannel = createMemo(() => hasPermission("create_message"));
  const canPublishVoiceCamera = createMemo(() => hasPermission("publish_video"));
  const canPublishVoiceScreenShare = createMemo(() => hasPermission("publish_screen_share"));
  const canSubscribeVoiceStreams = createMemo(() => hasPermission("subscribe_streams"));
  const canManageWorkspaceChannels = createMemo(() => {
    const role = channelPermissions()?.role;
    return canDiscoverWorkspaceOperation(role);
  });
  const canManageSearchMaintenance = createMemo(() => canManageWorkspaceChannels());
  const canManageRoles = createMemo(() => hasPermission("manage_roles"));
  const canManageChannelOverrides = createMemo(() => hasPermission("manage_channel_overrides"));
  const canBanMembers = createMemo(() => hasPermission("ban_member"));
  const canDeleteMessages = createMemo(() => hasPermission("delete_message"));
  const hasModerationAccess = createMemo(
    () => canManageRoles() || canBanMembers() || canManageChannelOverrides(),
  );
  const canDismissWorkspaceCreateForm = createMemo(() => workspaces().length > 0);

  const activeChannelKey = createMemo(() => {
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    return guildId && channelId ? channelKey(guildId, channelId) : null;
  });
  const activeVoiceSession = createMemo(() => {
    const key = voiceSessionChannelKey();
    return key ? parseChannelKey(key) : null;
  });
  const activeVoiceWorkspace = createMemo(() => {
    const voiceSession = activeVoiceSession();
    if (!voiceSession) {
      return null;
    }
    return workspaces().find((workspace) => workspace.guildId === voiceSession.guildId) ?? null;
  });
  const activeVoiceSessionChannel = createMemo(() => {
    const voiceSession = activeVoiceSession();
    const workspace = activeVoiceWorkspace();
    if (!voiceSession || !workspace) {
      return null;
    }
    return (
      workspace.channels.find((channel) => channel.channelId === voiceSession.channelId && channel.kind === "voice") ??
      null
    );
  });
  const activeVoiceSessionLabel = createMemo(() => {
    const workspace = activeVoiceWorkspace();
    const channel = activeVoiceSessionChannel();
    if (workspace && channel) {
      return `${channel.name} / ${workspace.guildName}`;
    }
    if (channel) {
      return channel.name;
    }
    return "Unknown voice room";
  });

  const activeAttachments = createMemo(() => {
    const key = activeChannelKey();
    if (!key) {
      return [];
    }
    return attachmentByChannel()[key] ?? [];
  });
  const voiceConnectionState = createMemo(() => voiceConnectionLabel(rtcSnapshot()));
  const isVoiceSessionActive = createMemo(() => {
    const state = rtcSnapshot().connectionStatus;
    return state === "connecting" || state === "connected" || state === "reconnecting";
  });
  const isVoiceSessionForActiveChannel = createMemo(() => {
    const key = activeChannelKey();
    return Boolean(key) && key === voiceSessionChannelKey() && isVoiceSessionActive();
  });
  const isVoiceSessionForChannel = (channelId: ChannelId): boolean => {
    const guildId = activeGuildId();
    if (!guildId || !isVoiceSessionActive()) {
      return false;
    }
    return voiceSessionChannelKey() === channelKey(guildId, channelId);
  };
  const hasVoicePublishGrant = (source: MediaPublishSource): boolean =>
    voiceSessionCapabilities().publishSources.includes(source);
  const canToggleVoiceCamera = createMemo(
    () =>
      isVoiceSessionActive() &&
      canPublishVoiceCamera() &&
      hasVoicePublishGrant("camera"),
  );
  const canToggleVoiceScreenShare = createMemo(
    () =>
      isVoiceSessionActive() &&
      canPublishVoiceScreenShare() &&
      hasVoicePublishGrant("screen_share"),
  );
  const canShowVoiceHeaderControls = createMemo(
    () => isActiveVoiceChannel() && canAccessActiveChannel(),
  );
  const voiceRosterEntries = createMemo<VoiceRosterEntry[]>(() => {
    const snapshot = rtcSnapshot();
    const entries: VoiceRosterEntry[] = [];
    const seenIdentities = new Set<string>();
    const activeSpeakers = new Set(snapshot.activeSpeakerIdentities);
    const identitiesWithCamera = new Set<string>();
    const identitiesWithScreenShare = new Set<string>();
    for (const track of snapshot.videoTracks) {
      if (track.source === "camera") {
        identitiesWithCamera.add(track.participantIdentity);
      } else if (track.source === "screen_share") {
        identitiesWithScreenShare.add(track.participantIdentity);
      }
    }
    const localIdentity = snapshot.localParticipantIdentity;
    if (localIdentity) {
      entries.push({
        identity: localIdentity,
        isLocal: true,
        isSpeaking: activeSpeakers.has(localIdentity),
        hasCamera: identitiesWithCamera.has(localIdentity),
        hasScreenShare: identitiesWithScreenShare.has(localIdentity),
      });
      seenIdentities.add(localIdentity);
    }
    for (const participant of snapshot.participants) {
      if (seenIdentities.has(participant.identity)) {
        continue;
      }
      entries.push({
        identity: participant.identity,
        isLocal: false,
        isSpeaking: activeSpeakers.has(participant.identity),
        hasCamera: identitiesWithCamera.has(participant.identity),
        hasScreenShare: identitiesWithScreenShare.has(participant.identity),
      });
      seenIdentities.add(participant.identity);
    }
    return entries;
  });
  const voiceStreamPermissionHints = createMemo(() => {
    if (!isVoiceSessionForActiveChannel()) {
      return [];
    }
    const hints: string[] = [];
    if (!canPublishVoiceCamera()) {
      hints.push("Camera disabled: channel permission publish_video is missing.");
    } else if (!hasVoicePublishGrant("camera")) {
      hints.push("Camera disabled: this voice token did not grant camera publish.");
    }
    if (!canPublishVoiceScreenShare()) {
      hints.push("Screen share disabled: channel permission publish_screen_share is missing.");
    } else if (!hasVoicePublishGrant("screen_share")) {
      hints.push("Screen share disabled: this voice token did not grant screen publish.");
    }
    if (!canSubscribeVoiceStreams()) {
      hints.push("Remote stream subscription is denied by channel permission.");
    } else if (!voiceSessionCapabilities().canSubscribe) {
      hints.push("Remote stream subscription is denied for this call.");
    }
    return hints;
  });
  const voiceSessionDurationLabel = createMemo(() => {
    if (!isVoiceSessionActive()) {
      return "0:00";
    }
    const startedAt = voiceSessionStartedAtUnixMs();
    if (!startedAt) {
      return "0:00";
    }
    const elapsedSeconds = Math.floor((voiceDurationClockUnixMs() - startedAt) / 1000);
    return formatVoiceDuration(elapsedSeconds);
  });

  const canCloseActivePanel = createMemo(() => {
    if (activeOverlayPanel() !== "workspace-create") {
      return true;
    }
    return canDismissWorkspaceCreateForm();
  });

  const openSettingsCategory = (category: SettingsCategory): void => {
    setActiveSettingsCategory(category);
    if (category === "voice") {
      setActiveVoiceSettingsSubmenu("audio-devices");
    }
  };

  const persistVoiceDevicePreferences = (next: VoiceDevicePreferences): void => {
    setVoiceDevicePreferences(next);
    try {
      saveVoiceDevicePreferences(next);
    } catch {
      setAudioDevicesError("Unable to persist audio device preferences in local storage.");
    }
  };

  const refreshAudioDeviceInventory = async (): Promise<void> => {
    if (isRefreshingAudioDevices()) {
      return;
    }
    setRefreshingAudioDevices(true);
    setAudioDevicesError("");
    try {
      const inventory = await enumerateAudioDevices();
      setAudioInputDevices(inventory.audioInputs);
      setAudioOutputDevices(inventory.audioOutputs);
      setAudioDevicesStatus(
        `Detected ${inventory.audioInputs.length} microphone(s) and ${inventory.audioOutputs.length} speaker(s).`,
      );
      const current = voiceDevicePreferences();
      const reconciled = reconcileVoiceDevicePreferences(current, inventory);
      if (
        current.audioInputDeviceId !== reconciled.audioInputDeviceId ||
        current.audioOutputDeviceId !== reconciled.audioOutputDeviceId
      ) {
        persistVoiceDevicePreferences(reconciled);
        setAudioDevicesStatus(
          "Some saved audio devices are no longer available. Reverted to system defaults.",
        );
      }
    } catch (error) {
      setAudioInputDevices([]);
      setAudioOutputDevices([]);
      setAudioDevicesStatus("");
      setAudioDevicesError(mapError(error, "Unable to enumerate audio devices."));
    } finally {
      setRefreshingAudioDevices(false);
    }
  };

  const setVoiceDevicePreference = async (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ): Promise<void> => {
    const options = kind === "audioinput" ? audioInputDevices() : audioOutputDevices();
    if (nextValue.length > 0 && !options.some((entry) => entry.deviceId === nextValue)) {
      setAudioDevicesError(
        kind === "audioinput"
          ? "Selected microphone is not available."
          : "Selected speaker is not available.",
      );
      return;
    }

    const nextDeviceId = nextValue.length > 0 ? (nextValue as MediaDeviceId) : null;
    const next: VoiceDevicePreferences =
      kind === "audioinput"
        ? {
            ...voiceDevicePreferences(),
            audioInputDeviceId: nextDeviceId,
          }
        : {
            ...voiceDevicePreferences(),
            audioOutputDeviceId: nextDeviceId,
          };
    setAudioDevicesError("");
    persistVoiceDevicePreferences(next);

    if (!rtcClient || !isVoiceSessionActive()) {
      setAudioDevicesStatus(
        kind === "audioinput"
          ? "Microphone preference saved for the next voice join."
          : "Speaker preference saved for the next voice join.",
      );
      return;
    }

    try {
      if (kind === "audioinput") {
        await rtcClient.setAudioInputDevice(next.audioInputDeviceId);
      } else {
        await rtcClient.setAudioOutputDevice(next.audioOutputDeviceId);
      }
      if (nextDeviceId) {
        setAudioDevicesStatus(
          kind === "audioinput"
            ? "Microphone updated for the active voice session."
            : "Speaker updated for the active voice session.",
        );
      } else {
        setAudioDevicesStatus(
          kind === "audioinput"
            ? "Microphone preference cleared. Current session keeps its current device."
            : "Speaker preference cleared. Current session keeps its current device.",
        );
      }
    } catch (error) {
      setAudioDevicesError(
        mapRtcError(
          error,
          kind === "audioinput"
            ? "Unable to apply microphone selection."
            : "Unable to apply speaker selection.",
        ),
      );
    }
  };

  const openOverlayPanel = (panel: OverlayPanel) => {
    if (panel === "workspace-create") {
      setWorkspaceError("");
    }
    if (panel === "channel-create") {
      setChannelCreateError("");
    }
    if (panel === "settings") {
      openSettingsCategory("voice");
    }
    setActiveOverlayPanel(panel);
  };

  const closeOverlayPanel = () => {
    if (!canCloseActivePanel()) {
      return;
    }
    setActiveOverlayPanel(null);
  };

  const overlayPanelTitle = (panel: OverlayPanel): string => {
    switch (panel) {
      case "workspace-create":
        return "Create workspace";
      case "channel-create":
        return "Create channel";
      case "settings":
        return "Settings";
      case "public-directory":
        return "Public workspace directory";
      case "friendships":
        return "Friendships";
      case "search":
        return "Search";
      case "attachments":
        return "Attachments";
      case "moderation":
        return "Moderation";
      case "utility":
        return "Utility";
    }
  };

  const overlayPanelClassName = (panel: OverlayPanel): string => {
    if (panel === "workspace-create" || panel === "channel-create") {
      return "panel-window panel-window-compact";
    }
    if (panel === "settings" || panel === "public-directory" || panel === "friendships") {
      return "panel-window panel-window-medium";
    }
    return "panel-window";
  };

  const ensureRtcClient = (): RtcClient => {
    if (rtcClient) {
      return rtcClient;
    }
    rtcClient = createRtcClient();
    stopRtcSubscription = rtcClient.subscribe((snapshot) => {
      setRtcSnapshot(snapshot);
    });
    return rtcClient;
  };

  const releaseRtcClient = async (): Promise<void> => {
    if (stopRtcSubscription) {
      stopRtcSubscription();
      stopRtcSubscription = null;
    }
    if (rtcClient) {
      try {
        await rtcClient.destroy();
      } catch {
        // Deterministic local teardown even if remote transport cleanup fails.
      } finally {
        rtcClient = null;
      }
    }
    setRtcSnapshot(RTC_DISCONNECTED_SNAPSHOT);
    setVoiceSessionChannelKey(null);
    setVoiceSessionStartedAtUnixMs(null);
    setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
  };

  const actorLabel = (actorId: string): string => resolvedUsernames()[actorId] ?? shortActor(actorId);
  const displayUserLabel = (userId: string): string => actorLabel(userId);
  const voiceParticipantLabel = (identity: string, isLocal: boolean): string => {
    const label = actorLabel(identity);
    return isLocal ? `${label} (you)` : label;
  };

  const setReactionPending = (key: string, pending: boolean) => {
    setPendingReactionByKey((existing) => {
      if (pending) {
        if (existing[key]) {
          return existing;
        }
        return { ...existing, [key]: true };
      }
      if (!existing[key]) {
        return existing;
      }
      const next = { ...existing };
      delete next[key];
      return next;
    });
  };

  const clearReactionStateForMessage = (messageId: MessageId) => {
    const prefix = reactionPrefix(messageId);
    setReactionState((existing) => clearKeysByPrefix(existing, prefix));
    setPendingReactionByKey((existing) => clearKeysByPrefix(existing, prefix));
    if (openReactionPickerMessageId() === messageId) {
      setOpenReactionPickerMessageId(null);
    }
  };

  const loadPublicGuildDirectory = async (query?: string) => {
    const session = auth.session();
    if (!session) {
      setPublicGuildDirectory([]);
      return;
    }
    if (isSearchingPublicGuilds()) {
      return;
    }
    setSearchingPublicGuilds(true);
    setPublicGuildSearchError("");
    try {
      const directory = await fetchPublicGuildDirectory(session, {
        query,
        limit: 20,
      });
      setPublicGuildDirectory(directory.guilds);
    } catch (error) {
      setPublicGuildSearchError(mapError(error, "Unable to load public workspace directory."));
      setPublicGuildDirectory([]);
    } finally {
      setSearchingPublicGuilds(false);
    }
  };

  const refreshFriendDirectory = async () => {
    const session = auth.session();
    if (!session) {
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      return;
    }
    setFriendError("");
    try {
      const [friendList, requestList] = await Promise.all([
        fetchFriends(session),
        fetchFriendRequests(session),
      ]);
      setFriends(friendList);
      setFriendRequests(requestList);
    } catch (error) {
      setFriendError(mapError(error, "Unable to load friendship state."));
    }
  };

  const [profile] = createResource(async () => {
    const session = auth.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return fetchMe(session);
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      setWorkspaces([]);
      setChannelPermissions(null);
      setWorkspaceBootstrapDone(true);
      return;
    }

    let cancelled = false;
    setWorkspaceBootstrapDone(false);

    const bootstrap = async () => {
      try {
        const guilds = await fetchGuilds(session);
        const workspacesWithChannels = await Promise.all(
          guilds.map(async (guild) => {
            try {
              return {
                guildId: guild.guildId,
                guildName: guild.name,
                visibility: guild.visibility,
                channels: await fetchGuildChannels(session, guild.guildId),
              };
            } catch (error) {
              if (error instanceof ApiError && (error.code === "forbidden" || error.code === "not_found")) {
                return null;
              }
              throw error;
            }
          }),
        );
        if (cancelled) {
          return;
        }
        const filtered = workspacesWithChannels.filter(
          (workspace): workspace is WorkspaceRecord =>
            workspace !== null && workspace.channels.length > 0,
        );
        setWorkspaces(filtered);
        const selectedGuild = activeGuildId();
        const selectedWorkspace =
          (selectedGuild && filtered.find((workspace) => workspace.guildId === selectedGuild)) ??
          filtered[0] ??
          null;
        setActiveGuildId(selectedWorkspace?.guildId ?? null);
        const selectedChannel = activeChannelId();
        const nextChannel =
          (selectedChannel &&
            selectedWorkspace?.channels.find((channel) => channel.channelId === selectedChannel)) ??
          selectedWorkspace?.channels[0] ??
          null;
        setActiveChannelId(nextChannel?.channelId ?? null);
      } catch {
        if (!cancelled) {
          setWorkspaces([]);
          setActiveGuildId(null);
          setActiveChannelId(null);
        }
      } finally {
        if (!cancelled) {
          setWorkspaceBootstrapDone(true);
        }
      }
    };

    void bootstrap();

    onCleanup(() => {
      cancelled = true;
    });
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setPublicGuildDirectory([]);
      setPublicGuildSearchError("");
      return;
    }
    void untrack(() => loadPublicGuildDirectory());
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      setFriendStatus("");
      setFriendError("");
      return;
    }
    void untrack(() => refreshFriendDirectory());
  });

  createEffect(() => {
    const value = profile();
    if (!value) {
      return;
    }
    primeUsernameCache([{ userId: value.userId, username: value.username }]);
    setResolvedUsernames((existing) => ({
      ...existing,
      [value.userId]: value.username,
    }));
  });

  createEffect(() => {
    const known = [
      ...friends().map((friend) => ({
        userId: friend.userId,
        username: friend.username,
      })),
      ...friendRequests().incoming.map((request) => ({
        userId: request.senderUserId,
        username: request.senderUsername,
      })),
      ...friendRequests().outgoing.map((request) => ({
        userId: request.recipientUserId,
        username: request.recipientUsername,
      })),
    ];
    if (known.length === 0) {
      return;
    }
    primeUsernameCache(known);
    setResolvedUsernames((existing) => ({
      ...existing,
      ...Object.fromEntries(known.map((entry) => [entry.userId, entry.username])),
    }));
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      return;
    }

    const lookupIds = new Set<UserId>();
    for (const message of messages()) {
      lookupIds.add(message.authorId);
    }
    for (const memberId of onlineMembers()) {
      try {
        lookupIds.add(userIdFromInput(memberId));
      } catch {
        continue;
      }
    }
    for (const participant of voiceRosterEntries()) {
      try {
        lookupIds.add(userIdFromInput(participant.identity));
      } catch {
        continue;
      }
    }
    const result = searchResults();
    if (result) {
      for (const message of result.messages) {
        lookupIds.add(message.authorId);
      }
    }
    if (lookupIds.size === 0) {
      return;
    }

    let cancelled = false;
    const resolveVisibleUsernames = async () => {
      try {
        const resolved = await resolveUsernames(session, [...lookupIds]);
        if (cancelled || Object.keys(resolved).length === 0) {
          return;
        }
        setResolvedUsernames((existing) => ({
          ...existing,
          ...resolved,
        }));
      } catch {
        // Keep user-id fallback rendering if lookup fails.
      }
    };
    void resolveVisibleUsernames();

    onCleanup(() => {
      cancelled = true;
    });
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(workspaces());
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    if (workspaces().length === 0) {
      setActiveOverlayPanel("workspace-create");
    }
  });

  createEffect(() => {
    const selectedGuild = activeGuildId();
    if (!selectedGuild || !workspaces().some((workspace) => workspace.guildId === selectedGuild)) {
      setActiveGuildId(workspaces()[0]?.guildId ?? null);
      return;
    }

    const channel = activeChannelId();
    const workspace = workspaces().find((entry) => entry.guildId === selectedGuild);
    if (!workspace) {
      return;
    }
    if (!channel || !workspace.channels.some((entry) => entry.channelId === channel)) {
      setActiveChannelId(workspace.channels[0]?.channelId ?? null);
    }
  });

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setChannelPermissions(null);
      return;
    }

    let cancelled = false;
    const loadPermissions = async () => {
      try {
        const snapshot = await fetchChannelPermissionSnapshot(session, guildId, channelId);
        if (!cancelled) {
          setChannelPermissions(snapshot);
        }
      } catch (error) {
        if (!cancelled) {
          setChannelPermissions(null);
          if (error instanceof ApiError && (error.code === "forbidden" || error.code === "not_found")) {
            setWorkspaces((existing) =>
              existing
                .map((workspace) => {
                  if (workspace.guildId !== guildId) {
                    return workspace;
                  }
                  return {
                    ...workspace,
                    channels: workspace.channels.filter((channel) => channel.channelId !== channelId),
                  };
                })
                .filter((workspace) => workspace.channels.length > 0),
            );
          }
        }
      }
    };
    void loadPermissions();

    onCleanup(() => {
      cancelled = true;
    });
  });

  const refreshMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessages([]);
      setNextBefore(null);
      return;
    }

    setMessageError("");
    setLoadingMessages(true);
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, { limit: 50 });
      setMessages([...history.messages].reverse());
      setNextBefore(history.nextBefore);
      setEditingMessageId(null);
      setEditingDraft("");
    } catch (error) {
      setMessageError(mapError(error, "Unable to load messages."));
      setMessages([]);
      setNextBefore(null);
    } finally {
      setLoadingMessages(false);
    }
  };

  const loadOlderMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const before = nextBefore();
    if (!session || !guildId || !channelId || !before || isLoadingOlder()) {
      return;
    }

    setLoadingOlder(true);
    setMessageError("");
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, {
        limit: 50,
        before,
      });
      const olderAscending = [...history.messages].reverse();
      setMessages((existing) => prependOlderMessages(existing, olderAscending));
      setNextBefore(history.nextBefore);
    } catch (error) {
      setMessageError(mapError(error, "Unable to load older messages."));
    } finally {
      setLoadingOlder(false);
    }
  };

  createEffect(() => {
    void activeGuildId();
    void activeChannelId();
    const canRead = canAccessActiveChannel();
    setReactionState({});
    setPendingReactionByKey({});
    setOpenReactionPickerMessageId(null);
    setSearchResults(null);
    setSearchError("");
    setSearchOpsStatus("");
    setAttachmentStatus("");
    setAttachmentError("");
    setVoiceStatus("");
    setVoiceError("");
    if (canRead) {
      void refreshMessages();
    } else {
      setMessages([]);
      setNextBefore(null);
    }
  });

  createEffect(() => {
    void mediaPreviewRetryTick();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const messageList = messages();
    if (!session || !guildId || !channelId) {
      setMessageMediaByAttachmentId((existing) => {
        for (const preview of Object.values(existing)) {
          revokeObjectUrl(preview.url);
        }
        return {};
      });
      setLoadingMediaPreviewIds({});
      setFailedMediaPreviewIds({});
      previewRetryAttempts.clear();
      return;
    }

    const previewTargets = new Map<AttachmentId, AttachmentRecord>();
    for (const message of messageList) {
      for (const attachment of message.attachments) {
        const { kind } = resolveAttachmentPreviewType(null, attachment.mimeType, attachment.filename);
        if (kind === "file" || attachment.sizeBytes > MAX_EMBED_PREVIEW_BYTES) {
          continue;
        }
        previewTargets.set(attachment.attachmentId, attachment);
      }
    }

    const existingPreviews = untrack(() => messageMediaByAttachmentId());
    const targetIds = new Set<string>([...previewTargets.keys()]);
    setMessageMediaByAttachmentId((existing) => {
      const next: Record<string, MessageMediaPreview> = {};
      for (const [attachmentId, preview] of Object.entries(existing)) {
        if (targetIds.has(attachmentId)) {
          next[attachmentId] = preview;
        } else {
          revokeObjectUrl(preview.url);
          previewRetryAttempts.delete(attachmentId);
        }
      }
      return next;
    });
    setLoadingMediaPreviewIds((existing) =>
      Object.fromEntries(
        Object.entries(existing).filter(([attachmentId]) => targetIds.has(attachmentId)),
      ) as Record<string, true>,
    );
    setFailedMediaPreviewIds((existing) =>
      Object.fromEntries(
        Object.entries(existing).filter(([attachmentId]) => targetIds.has(attachmentId)),
      ) as Record<string, true>,
    );

    let cancelled = false;
    const refreshSessionForPreview = async (): Promise<void> => {
      if (previewSessionRefreshPromise) {
        return previewSessionRefreshPromise;
      }
      const current = auth.session();
      if (!current) {
        throw new Error("missing_session");
      }
      previewSessionRefreshPromise = (async () => {
        const next = await refreshAuthSession(current.refreshToken);
        auth.setAuthenticatedSession(next);
      })();
      try {
        await previewSessionRefreshPromise;
      } finally {
        previewSessionRefreshPromise = null;
      }
    };

    for (const [attachmentId, attachment] of previewTargets) {
      if (existingPreviews[attachmentId] || inflightMessageMediaLoads.has(attachmentId)) {
        continue;
      }
      inflightMessageMediaLoads.add(attachmentId);
      setLoadingMediaPreviewIds((existing) => ({
        ...existing,
        [attachmentId]: true,
      }));
      setFailedMediaPreviewIds((existing) => {
        if (!existing[attachmentId]) {
          return existing;
        }
        const next = { ...existing };
        delete next[attachmentId];
        return next;
      });
      const attempt = previewRetryAttempts.get(attachmentId) ?? 0;
      const runFetch = async () => {
        let activeSession = auth.session() ?? session;
        try {
          return await downloadChannelAttachmentPreview(
            activeSession,
            guildId,
            channelId,
            attachmentId,
          );
        } catch (error) {
          if (
            error instanceof ApiError &&
            error.code === "invalid_credentials" &&
            attempt === 0
          ) {
            await refreshSessionForPreview();
            activeSession = auth.session() ?? activeSession;
            return downloadChannelAttachmentPreview(
              activeSession,
              guildId,
              channelId,
              attachmentId,
            );
          }
          throw error;
        }
      };
      const processFetch = () =>
        runFetch()
        .then((payload) => {
          if (cancelled) {
            return;
          }
          const { mimeType, kind } = resolveAttachmentPreviewType(
            payload.mimeType,
            attachment.mimeType,
            attachment.filename,
          );
          if (kind === "file") {
            setLoadingMediaPreviewIds((existing) => {
              const next = { ...existing };
              delete next[attachmentId];
              return next;
            });
            return;
          }
          const blob = new Blob([payload.bytes.buffer as ArrayBuffer], { type: mimeType });
          const url = createObjectUrl(blob);
          if (!url) {
            setLoadingMediaPreviewIds((existing) => {
              const next = { ...existing };
              delete next[attachmentId];
              return next;
            });
            setFailedMediaPreviewIds((existing) => ({
              ...existing,
              [attachmentId]: true,
            }));
            return;
          }
          setMessageMediaByAttachmentId((existing) => {
            const previous = existing[attachmentId];
            if (previous) {
              revokeObjectUrl(previous.url);
            }
            return {
              ...existing,
              [attachmentId]: {
                url,
                kind,
                mimeType,
              },
            };
          });
          previewRetryAttempts.delete(attachmentId);
          setLoadingMediaPreviewIds((existing) => {
            const next = { ...existing };
            delete next[attachmentId];
            return next;
          });
        })
        .catch(() => {
          if (cancelled) {
            return;
          }
          const nextAttempt = (previewRetryAttempts.get(attachmentId) ?? 0) + 1;
          previewRetryAttempts.set(attachmentId, nextAttempt);
          if (nextAttempt <= MAX_MEDIA_PREVIEW_RETRIES) {
            window.setTimeout(() => {
              setMediaPreviewRetryTick((value) => value + 1);
            }, 600 * nextAttempt);
            return;
          }
          setLoadingMediaPreviewIds((existing) => {
            const next = { ...existing };
            delete next[attachmentId];
            return next;
          });
          setFailedMediaPreviewIds((existing) => ({
            ...existing,
            [attachmentId]: true,
          }));
        })
        .finally(() => {
          inflightMessageMediaLoads.delete(attachmentId);
        });
      if (attempt === 0) {
        window.setTimeout(() => {
          if (cancelled) {
            inflightMessageMediaLoads.delete(attachmentId);
            return;
          }
          void processFetch();
        }, INITIAL_MEDIA_PREVIEW_DELAY_MS);
      } else {
        void processFetch();
      }
    }

    onCleanup(() => {
      cancelled = true;
    });
  });

  onCleanup(() => {
    for (const preview of Object.values(messageMediaByAttachmentId())) {
      revokeObjectUrl(preview.url);
    }
    setMessageMediaByAttachmentId({});
    setLoadingMediaPreviewIds({});
    setFailedMediaPreviewIds({});
  });

  const retryMediaPreview = (attachmentId: AttachmentId) => {
    previewRetryAttempts.delete(attachmentId);
    setFailedMediaPreviewIds((existing) => {
      const next = { ...existing };
      delete next[attachmentId];
      return next;
    });
    setMediaPreviewRetryTick((value) => value + 1);
  };

  createEffect(() => {
    const panel = activeOverlayPanel();
    if (!panel) {
      return;
    }

    const needsChannelAccess =
      panel === "channel-create" ||
      panel === "search" ||
      panel === "attachments" ||
      panel === "moderation";
    if (needsChannelAccess && !canAccessActiveChannel()) {
      setActiveOverlayPanel(null);
      return;
    }

    if (panel === "channel-create" && !canManageWorkspaceChannels()) {
      setActiveOverlayPanel(null);
      return;
    }

    if (panel === "moderation" && !hasModerationAccess()) {
      setActiveOverlayPanel(null);
    }
  });

  createEffect(() => {
    if (!activeOverlayPanel()) {
      return;
    }

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeOverlayPanel();
      }
    };

    window.addEventListener("keydown", onKeydown);
    onCleanup(() => window.removeEventListener("keydown", onKeydown));
  });

  createEffect(() => {
    const isVoiceAudioSettingsOpen =
      activeOverlayPanel() === "settings" &&
      activeSettingsCategory() === "voice" &&
      activeVoiceSettingsSubmenu() === "audio-devices";
    if (!isVoiceAudioSettingsOpen) {
      return;
    }
    void refreshAudioDeviceInventory();
  });

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || !canAccessActiveChannel()) {
      setGatewayOnline(false);
      setOnlineMembers([]);
      return;
    }

    const gateway = connectGateway(session.accessToken, guildId, channelId, {
      onOpenStateChange: (isOpen) => setGatewayOnline(isOpen),
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        setMessages((existing) => mergeMessage(existing, message));
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers((existing) => {
          if (payload.status === "online") {
            return existing.includes(payload.userId) ? existing : [...existing, payload.userId];
          }
          return existing.filter((entry) => entry !== payload.userId);
        });
      },
    });

    onCleanup(() => gateway.close());
  });

  createEffect(() => {
    if (!isVoiceSessionActive() || !voiceSessionStartedAtUnixMs()) {
      return;
    }
    setVoiceDurationClockUnixMs(Date.now());
    const timer = window.setInterval(() => {
      setVoiceDurationClockUnixMs(Date.now());
    }, 1000);
    onCleanup(() => window.clearInterval(timer));
  });

  createEffect(() => {
    const session = auth.session();
    const connectedChannelKey = voiceSessionChannelKey();
    if (!connectedChannelKey || isLeavingVoice()) {
      return;
    }
    if (!session) {
      void leaveVoiceChannel();
      return;
    }
    const connected = parseChannelKey(connectedChannelKey);
    if (!connected) {
      void leaveVoiceChannel();
      return;
    }
    const workspace = workspaces().find((entry) => entry.guildId === connected.guildId) ?? null;
    const voiceChannelStillVisible = workspace?.channels.some(
      (channel) => channel.channelId === connected.channelId && channel.kind === "voice",
    );
    if (!workspace || !voiceChannelStillVisible) {
      void leaveVoiceChannel();
    }
  });

  createEffect(() => {
    const snapshot = rtcSnapshot();
    const connectedChannelKey = voiceSessionChannelKey();
    const isJoining = isJoiningVoice();
    const isLeaving = isLeavingVoice();

    if (connectedChannelKey && snapshot.connectionStatus === "reconnecting") {
      setVoiceStatus("Voice reconnecting. Media may recover automatically.");
      setVoiceError("");
    }

    if (
      connectedChannelKey &&
      snapshot.connectionStatus === "connected" &&
      previousVoiceConnectionStatus === "reconnecting"
    ) {
      setVoiceStatus("Voice reconnected.");
      setVoiceError("");
    }

    if (
      connectedChannelKey &&
      snapshot.connectionStatus === "disconnected" &&
      previousVoiceConnectionStatus !== "disconnected" &&
      !isJoining &&
      !isLeaving
    ) {
      setVoiceSessionChannelKey(null);
      setVoiceSessionStartedAtUnixMs(null);
      setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
      setVoiceStatus("");
      setVoiceError("Voice connection dropped. Select Join Voice to reconnect.");
    }

    previousVoiceConnectionStatus = snapshot.connectionStatus;
  });

  onCleanup(() => {
    void releaseRtcClient();
  });

  const createWorkspace = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session) {
      setWorkspaceError("Missing auth session.");
      return;
    }
    if (isCreatingWorkspace()) {
      return;
    }

    setWorkspaceError("");
    setCreatingWorkspace(true);
    try {
      const guild = await createGuild(session, {
        name: guildNameFromInput(createGuildName()),
        visibility: guildVisibilityFromInput(createGuildVisibility()),
      });
      const channel = await createChannel(session, guild.guildId, {
        name: channelNameFromInput(createChannelName()),
        kind: channelKindFromInput(createChannelKind()),
      });
      const createdWorkspace: WorkspaceRecord = {
        guildId: guild.guildId,
        guildName: guild.name,
        visibility: guild.visibility,
        channels: [channel],
      };
      setWorkspaces((existing) => [...existing, createdWorkspace]);
      setActiveGuildId(createdWorkspace.guildId);
      setActiveChannelId(channel.channelId);
      setCreateChannelKind("text");
      setMessageStatus("Workspace created.");
      setActiveOverlayPanel(null);
    } catch (error) {
      setWorkspaceError(mapError(error, "Unable to create workspace."));
    } finally {
      setCreatingWorkspace(false);
    }
  };

  const createNewChannel = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setChannelCreateError("Select a workspace first.");
      return;
    }
    if (isCreatingChannel()) {
      return;
    }

    setChannelCreateError("");
    setCreatingChannel(true);
    try {
      const created = await createChannel(session, guildId, {
        name: channelNameFromInput(newChannelName()),
        kind: channelKindFromInput(newChannelKind()),
      });
      setWorkspaces((existing) =>
        upsertWorkspace(existing, guildId, (workspace) => {
          if (workspace.channels.some((channel) => channel.channelId === created.channelId)) {
            return workspace;
          }
          return {
            ...workspace,
            channels: [...workspace.channels, created],
          };
        }),
      );
      setActiveChannelId(created.channelId);
      setActiveOverlayPanel(null);
      setNewChannelName("backend");
      setNewChannelKind("text");
      setMessageStatus("Channel created.");
    } catch (error) {
      setChannelCreateError(mapError(error, "Unable to create channel."));
    } finally {
      setCreatingChannel(false);
    }
  };

  const sendMessage = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessageError("Select a channel first.");
      return;
    }

    if (isSendingMessage()) {
      return;
    }

    setMessageError("");
    setMessageStatus("");
    setSendingMessage(true);
    let uploadedForMessage: AttachmentRecord[] = [];
    try {
      const draft = composer().trim();
      const selectedFiles = composerAttachments();
      if (draft.length === 0 && selectedFiles.length === 0) {
        setMessageError("Message must include text or at least one attachment.");
        return;
      }
      for (const file of selectedFiles) {
        const filename = attachmentFilenameFromInput(file.name);
        const uploaded = await uploadChannelAttachment(session, guildId, channelId, file, filename);
        uploadedForMessage.push(uploaded);
      }

      const created = await createChannelMessage(session, guildId, channelId, {
        content: messageContentFromInput(draft),
        attachmentIds:
          uploadedForMessage.length > 0
            ? uploadedForMessage.map((record) => record.attachmentId)
            : undefined,
      });
      setMessages((existing) => mergeMessage(existing, created));
      setComposer("");
      setComposerAttachments([]);
      if (composerAttachmentInputRef) {
        composerAttachmentInputRef.value = "";
      }
      if (uploadedForMessage.length > 0) {
        const key = channelKey(guildId, channelId);
        setAttachmentByChannel((existing) => {
          const current = existing[key] ?? [];
          const uploadedIds = new Set(uploadedForMessage.map((record) => record.attachmentId));
          const deduped = current.filter((entry) => !uploadedIds.has(entry.attachmentId));
          return {
            ...existing,
            [key]: [...uploadedForMessage, ...deduped],
          };
        });
        setMessageStatus(
          `Sent with ${uploadedForMessage.length} attachment${uploadedForMessage.length === 1 ? "" : "s"}.`,
        );
      }
    } catch (error) {
      if (uploadedForMessage.length > 0) {
        await Promise.allSettled(
          uploadedForMessage.map((record) =>
            deleteChannelAttachment(session, guildId, channelId, record.attachmentId),
          ),
        );
      }
      setMessageError(mapError(error, "Unable to send message."));
    } finally {
      setSendingMessage(false);
    }
  };

  const openComposerAttachmentPicker = () => {
    if (!activeChannel() || !canAccessActiveChannel()) {
      setMessageError("Select a channel first.");
      return;
    }
    composerAttachmentInputRef?.click();
  };

  const onComposerAttachmentInput = (event: InputEvent & { currentTarget: HTMLInputElement }) => {
    const incomingFiles = [...(event.currentTarget.files ?? [])];
    if (incomingFiles.length === 0) {
      return;
    }

    setMessageError("");
    const existing = composerAttachments();
    const existingKeys = new Set(
      existing.map((file) => `${file.name}:${file.size}:${file.lastModified}:${file.type}`),
    );
    const next = [...existing];
    let reachedCap = existing.length >= MAX_COMPOSER_ATTACHMENTS;
    for (const file of incomingFiles) {
      const dedupeKey = `${file.name}:${file.size}:${file.lastModified}:${file.type}`;
      if (existingKeys.has(dedupeKey)) {
        continue;
      }
      if (next.length >= MAX_COMPOSER_ATTACHMENTS) {
        reachedCap = true;
        break;
      }
      next.push(file);
      existingKeys.add(dedupeKey);
    }
    setComposerAttachments(next);

    if (composerAttachmentInputRef) {
      composerAttachmentInputRef.value = "";
    }
    if (reachedCap) {
      setMessageError(`Maximum ${MAX_COMPOSER_ATTACHMENTS} attachments per message.`);
    }
  };

  const removeComposerAttachment = (target: File) => {
    setComposerAttachments((existing) =>
      existing.filter(
        (file) =>
          !(
            file.name === target.name &&
            file.size === target.size &&
            file.lastModified === target.lastModified &&
            file.type === target.type
          ),
      ),
    );
  };

  const beginEditMessage = (message: MessageRecord) => {
    setEditingMessageId(message.messageId);
    setEditingDraft(message.content);
  };

  const cancelEditMessage = () => {
    setEditingMessageId(null);
    setEditingDraft("");
  };

  const saveEditMessage = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || isSavingEdit()) {
      return;
    }

    setSavingEdit(true);
    setMessageError("");
    try {
      const updated = await editChannelMessage(session, guildId, channelId, messageId, {
        content: messageContentFromInput(editingDraft()),
      });
      setMessages((existing) => mergeMessage(existing, updated));
      setEditingMessageId(null);
      setEditingDraft("");
      setMessageStatus("Message updated.");
    } catch (error) {
      setMessageError(mapError(error, "Unable to edit message."));
    } finally {
      setSavingEdit(false);
    }
  };

  const removeMessage = async (messageId: MessageId) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || deletingMessageId()) {
      return;
    }

    setDeletingMessageId(messageId);
    setMessageError("");
    try {
      await deleteChannelMessage(session, guildId, channelId, messageId);
      setMessages((existing) => existing.filter((entry) => entry.messageId !== messageId));
      if (editingMessageId() === messageId) {
        cancelEditMessage();
      }
      clearReactionStateForMessage(messageId);
      setMessageStatus("Message deleted.");
    } catch (error) {
      setMessageError(mapError(error, "Unable to delete message."));
    } finally {
      setDeletingMessageId(null);
    }
  };

  const toggleReactionPicker = (messageId: MessageId) => {
    setOpenReactionPickerMessageId((existing) => (existing === messageId ? null : messageId));
  };

  const toggleMessageReaction = async (messageId: MessageId, emoji: ReactionEmoji) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, emoji);
    if (pendingReactionByKey()[key]) {
      return;
    }
    setReactionPending(key, true);
    const state = reactionState()[key] ?? { count: 0, reacted: false };

    try {
      if (state.reacted) {
        const response = await removeMessageReaction(session, guildId, channelId, messageId, emoji);
        setReactionState((existing) =>
          upsertReactionEntry(existing, key, {
            count: response.count,
            reacted: false,
          }),
        );
      } else {
        const response = await addMessageReaction(session, guildId, channelId, messageId, emoji);
        setReactionState((existing) =>
          upsertReactionEntry(existing, key, {
            count: response.count,
            reacted: true,
          }),
        );
      }
    } catch (error) {
      setMessageError(mapError(error, "Unable to update reaction."));
    } finally {
      setReactionPending(key, false);
    }
  };

  const addReactionFromPicker = async (messageId: MessageId, emoji: ReactionEmoji) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, emoji);
    if (pendingReactionByKey()[key]) {
      return;
    }
    setReactionPending(key, true);
    setOpenReactionPickerMessageId(null);

    try {
      const response = await addMessageReaction(session, guildId, channelId, messageId, emoji);
      setReactionState((existing) =>
        upsertReactionEntry(existing, key, {
          count: response.count,
          reacted: true,
        }),
      );
    } catch (error) {
      setMessageError(mapError(error, "Unable to update reaction."));
    } finally {
      setReactionPending(key, false);
    }
  };

  const runSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setSearchError("Select a workspace first.");
      return;
    }

    if (isSearching()) {
      return;
    }

    setSearching(true);
    setSearchError("");
    try {
      const results = await searchGuildMessages(session, guildId, {
        query: searchQueryFromInput(searchQuery()),
        limit: 20,
        channelId: activeChannelId() ?? undefined,
      });
      setSearchResults(results);
    } catch (error) {
      setSearchError(mapError(error, "Search request failed."));
      setSearchResults(null);
    } finally {
      setSearching(false);
    }
  };

  const runPublicGuildSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    await loadPublicGuildDirectory(publicGuildSearchQuery());
  };

  const submitFriendRequest = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      const recipientUserId = userIdFromInput(friendRecipientUserIdInput().trim());
      await createFriendRequest(session, recipientUserId);
      setFriendRecipientUserIdInput("");
      await refreshFriendDirectory();
      setFriendStatus("Friend request sent.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to create friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const acceptIncomingFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await acceptFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request accepted.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to accept friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const dismissFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await deleteFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const removeFriendship = async (friendUserId: UserId) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await removeFriend(session, friendUserId);
      await refreshFriendDirectory();
      setFriendStatus("Friend removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const rebuildSearch = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isRunningSearchOps()) {
      return;
    }

    setRunningSearchOps(true);
    setSearchError("");
    setSearchOpsStatus("");
    try {
      await rebuildGuildSearchIndex(session, guildId);
      setSearchOpsStatus("Search index rebuild queued.");
    } catch (error) {
      setSearchError(mapError(error, "Unable to rebuild search index."));
    } finally {
      setRunningSearchOps(false);
    }
  };

  const reconcileSearch = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isRunningSearchOps()) {
      return;
    }

    setRunningSearchOps(true);
    setSearchError("");
    setSearchOpsStatus("");
    try {
      const result = await reconcileGuildSearchIndex(session, guildId);
      setSearchOpsStatus(`Reconciled search index (upserted ${result.upserted}, deleted ${result.deleted}).`);
    } catch (error) {
      setSearchError(mapError(error, "Unable to reconcile search index."));
    } finally {
      setRunningSearchOps(false);
    }
  };

  const uploadAttachment = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const file = selectedAttachment();
    if (!session || !guildId || !channelId) {
      setAttachmentError("Select a channel first.");
      return;
    }
    if (!file) {
      setAttachmentError("Select a file to upload.");
      return;
    }
    if (isUploadingAttachment()) {
      return;
    }

    setAttachmentStatus("");
    setAttachmentError("");
    setUploadingAttachment(true);

    try {
      const filename = attachmentFilenameFromInput(
        attachmentFilename().trim().length > 0 ? attachmentFilename().trim() : file.name,
      );
      const uploaded = await uploadChannelAttachment(session, guildId, channelId, file, filename);
      const key = channelKey(guildId, channelId);
      setAttachmentByChannel((existing) => {
        const current = existing[key] ?? [];
        const deduped = current.filter((entry) => entry.attachmentId !== uploaded.attachmentId);
        return {
          ...existing,
          [key]: [uploaded, ...deduped],
        };
      });
      setAttachmentStatus(`Uploaded ${uploaded.filename} (${formatBytes(uploaded.sizeBytes)}).`);
      setSelectedAttachment(null);
      setAttachmentFilename("");
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to upload attachment."));
    } finally {
      setUploadingAttachment(false);
    }
  };

  const downloadAttachment = async (record: AttachmentRecord) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || downloadingAttachmentId()) {
      return;
    }

    setDownloadingAttachmentId(record.attachmentId);
    setAttachmentError("");
    try {
      const payload = await downloadChannelAttachment(session, guildId, channelId, record.attachmentId);
      const blob = new Blob([payload.bytes.buffer as ArrayBuffer], {
        type: payload.mimeType ?? record.mimeType,
      });
      const objectUrl = createObjectUrl(blob);
      if (!objectUrl) {
        throw new Error("missing_object_url");
      }
      const anchor = document.createElement("a");
      anchor.href = objectUrl;
      anchor.download = record.filename;
      anchor.rel = "noopener";
      anchor.click();
      window.setTimeout(() => revokeObjectUrl(objectUrl), 0);
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to download attachment."));
    } finally {
      setDownloadingAttachmentId(null);
    }
  };

  const removeAttachment = async (record: AttachmentRecord) => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || deletingAttachmentId()) {
      return;
    }

    setDeletingAttachmentId(record.attachmentId);
    setAttachmentError("");
    try {
      await deleteChannelAttachment(session, guildId, channelId, record.attachmentId);
      const key = channelKey(guildId, channelId);
      setAttachmentByChannel((existing) => ({
        ...existing,
        [key]: (existing[key] ?? []).filter((entry) => entry.attachmentId !== record.attachmentId),
      }));
      setAttachmentStatus(`Deleted ${record.filename}.`);
    } catch (error) {
      setAttachmentError(mapError(error, "Unable to delete attachment."));
    } finally {
      setDeletingAttachmentId(null);
    }
  };

  const leaveVoiceChannel = async (statusMessage?: string) => {
    if (isLeavingVoice()) {
      return;
    }
    setLeavingVoice(true);
    try {
      if (rtcClient) {
        await rtcClient.leave();
      }
    } catch {
      // Leave should still clear local voice state even if transport teardown fails.
    } finally {
      setVoiceSessionChannelKey(null);
      setVoiceSessionStartedAtUnixMs(null);
      setRtcSnapshot(RTC_DISCONNECTED_SNAPSHOT);
      setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
      if (statusMessage) {
        setVoiceStatus(statusMessage);
      }
      setLeavingVoice(false);
    }
  };

  const joinVoiceChannel = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channel = activeChannel();
    if (
      !session ||
      !guildId ||
      !channel ||
      channel.kind !== "voice" ||
      isJoiningVoice() ||
      isLeavingVoice()
    ) {
      return;
    }

    setJoiningVoice(true);
    setVoiceError("");
    setVoiceStatus("");
    setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
    try {
      const requestedPublishSources: MediaPublishSource[] = ["microphone"];
      if (canPublishVoiceCamera()) {
        requestedPublishSources.push("camera");
      }
      if (canPublishVoiceScreenShare()) {
        requestedPublishSources.push("screen_share");
      }
      const token = await issueVoiceToken(session, guildId, channel.channelId, {
        canSubscribe: canSubscribeVoiceStreams(),
        publishSources: requestedPublishSources,
      });
      const client = ensureRtcClient();
      const preferences = voiceDevicePreferences();
      await client.setAudioInputDevice(preferences.audioInputDeviceId);
      await client.setAudioOutputDevice(preferences.audioOutputDeviceId);
      await client.join({
        livekitUrl: token.livekitUrl,
        token: token.token,
      });
      setVoiceSessionChannelKey(channelKey(guildId, channel.channelId));
      setVoiceSessionStartedAtUnixMs(Date.now());
      setVoiceDurationClockUnixMs(Date.now());
      setVoiceSessionCapabilities({
        canSubscribe: token.canSubscribe,
        publishSources: [...token.publishSources],
      });
      const joinSnapshot = client.snapshot();
      if (joinSnapshot.lastErrorCode === "audio_device_switch_failed" && joinSnapshot.lastErrorMessage) {
        setAudioDevicesError(joinSnapshot.lastErrorMessage);
      }

      if (token.canPublish && token.publishSources.includes("microphone")) {
        try {
          await client.setMicrophoneEnabled(true);
          setVoiceStatus("Voice connected. Microphone enabled.");
        } catch (error) {
          setVoiceStatus("Voice connected.");
          setVoiceError(mapRtcError(error, "Connected, but microphone activation failed."));
        }
        return;
      }

      setVoiceStatus("Voice connected in listen-only mode.");
    } catch (error) {
      setVoiceError(mapVoiceJoinError(error));
    } finally {
      setJoiningVoice(false);
    }
  };

  const toggleVoiceMicrophone = async () => {
    if (!rtcClient || isTogglingVoiceMic()) {
      return;
    }
    setTogglingVoiceMic(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleMicrophone();
      setVoiceStatus(enabled ? "Microphone unmuted." : "Microphone muted.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update microphone."));
    } finally {
      setTogglingVoiceMic(false);
    }
  };

  const toggleVoiceCamera = async () => {
    if (!rtcClient || isTogglingVoiceCamera()) {
      return;
    }
    if (!canToggleVoiceCamera()) {
      setVoiceError("Camera publish is not allowed for this call.");
      return;
    }
    setTogglingVoiceCamera(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleCamera();
      setVoiceStatus(enabled ? "Camera enabled." : "Camera disabled.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update camera."));
    } finally {
      setTogglingVoiceCamera(false);
    }
  };

  const toggleVoiceScreenShare = async () => {
    if (!rtcClient || isTogglingVoiceScreenShare()) {
      return;
    }
    if (!canToggleVoiceScreenShare()) {
      setVoiceError("Screen share publish is not allowed for this call.");
      return;
    }
    setTogglingVoiceScreenShare(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleScreenShare();
      setVoiceStatus(enabled ? "Screen share enabled." : "Screen share stopped.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update screen share."));
    } finally {
      setTogglingVoiceScreenShare(false);
    }
  };

  const runModerationAction = async (
    action: (sessionUserId: UserId, sessionUsername: string) => Promise<void>,
  ) => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId || isModerating()) {
      return;
    }

    setModerationError("");
    setModerationStatus("");
    setModerating(true);
    try {
      const me = await fetchMe(session);
      await action(me.userId, me.username);
    } catch (error) {
      setModerationError(mapError(error, "Moderation action failed."));
    } finally {
      setModerating(false);
    }
  };

  const runMemberAction = async (action: "add" | "role" | "kick" | "ban") => {
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setModerationError("Select a workspace first.");
      return;
    }

    let targetUserId: UserId;
    try {
      targetUserId = userIdFromInput(moderationUserIdInput().trim());
    } catch (error) {
      setModerationError(mapError(error, "Target user ID is invalid."));
      return;
    }
    await runModerationAction(async () => {
      if (action === "add") {
        await addGuildMember(session, guildId, targetUserId);
        setModerationStatus("Member add request accepted.");
        return;
      }
      if (action === "role") {
        const role = roleFromInput(moderationRoleInput());
        await updateGuildMemberRole(session, guildId, targetUserId, role);
        setModerationStatus(`Member role updated to ${role}.`);
        return;
      }
      if (action === "kick") {
        await kickGuildMember(session, guildId, targetUserId);
        setModerationStatus("Member kicked.");
        return;
      }
      await banGuildMember(session, guildId, targetUserId);
      setModerationStatus("Member banned.");
    });
  };

  const applyOverride = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || isModerating()) {
      return;
    }

    try {
      const allow = parsePermissionCsv(overrideAllowCsv());
      const deny = parsePermissionCsv(overrideDenyCsv());
      if (allow.some((permission) => deny.includes(permission))) {
        throw new DomainValidationError("Allow and deny permission sets cannot overlap.");
      }

      setModerating(true);
      setModerationError("");
      setModerationStatus("");
      await setChannelRoleOverride(session, guildId, channelId, roleFromInput(overrideRoleInput()), {
        allow,
        deny,
      });
      setModerationStatus("Channel role override updated.");
    } catch (error) {
      setModerationError(mapError(error, "Unable to set channel override."));
    } finally {
      setModerating(false);
    }
  };

  const refreshSession = async () => {
    const session = auth.session();
    if (!session || isRefreshingSession()) {
      return;
    }

    setRefreshingSession(true);
    setSessionError("");
    setSessionStatus("");
    try {
      const next = await refreshAuthSession(session.refreshToken);
      auth.setAuthenticatedSession(next);
      setSessionStatus("Session refreshed.");
    } catch (error) {
      setSessionError(mapError(error, "Unable to refresh session."));
    } finally {
      setRefreshingSession(false);
    }
  };

  const logout = async () => {
    await leaveVoiceChannel();
    await releaseRtcClient();
    const session = auth.session();
    if (session) {
      try {
        await logoutAuthSession(session.refreshToken);
      } catch {
        // best-effort logout; local session will still be cleared
      }
    }
    auth.clearAuthenticatedSession();
    clearWorkspaceCache();
  };

  const runHealthCheck = async () => {
    if (isCheckingHealth()) {
      return;
    }
    setCheckingHealth(true);
    setDiagError("");
    try {
      const health = await fetchHealth();
      setHealthStatus(`Health: ${health.status}`);
    } catch (error) {
      setDiagError(mapError(error, "Health check failed."));
    } finally {
      setCheckingHealth(false);
    }
  };

  const runEcho = async (event: SubmitEvent) => {
    event.preventDefault();
    if (isEchoing()) {
      return;
    }

    setEchoing(true);
    setDiagError("");
    try {
      const echoed = await echoMessage({ message: echoInput() });
      setHealthStatus(`Echo: ${echoed.slice(0, 60)}`);
    } catch (error) {
      setDiagError(mapError(error, "Echo request failed."));
    } finally {
      setEchoing(false);
    }
  };

  return (
    <div class="app-shell-scaffold">
      <div
        classList={{
          "app-shell": true,
          "channel-rail-collapsed": isChannelRailCollapsed(),
          "member-rail-collapsed": isMemberRailCollapsed(),
        }}
      >
        <aside class="server-rail" aria-label="servers">
          <header class="rail-label">WS</header>
          <div class="server-list">
            <For each={workspaces()}>
              {(workspace) => (
                <button
                  title={`${workspace.guildName} (${workspace.visibility})`}
                  classList={{ active: activeGuildId() === workspace.guildId }}
                  onClick={() => {
                    setActiveGuildId(workspace.guildId);
                    setActiveChannelId(workspace.channels[0]?.channelId ?? null);
                  }}
                >
                  {workspace.guildName.slice(0, 1).toUpperCase()}
                </button>
              )}
            </For>
          </div>
          <div class="server-rail-footer">
            <button
              type="button"
              class="server-action"
              aria-label="Open workspace create panel"
              title="Create workspace"
              onClick={() => openOverlayPanel("workspace-create")}
              disabled={isCreatingWorkspace()}
            >
              +
            </button>
            <button
              type="button"
              class="server-action"
              aria-label="Open public workspace directory panel"
              title="Public workspace directory"
              onClick={() => openOverlayPanel("public-directory")}
            >
              D
            </button>
            <button
              type="button"
              class="server-action"
              aria-label="Open friendships panel"
              title="Friendships"
              onClick={() => openOverlayPanel("friendships")}
            >
              F
            </button>
            <button
              type="button"
              class="server-action"
              aria-label="Open settings panel"
              title="Settings"
              onClick={() => openOverlayPanel("settings")}
            >
              S
            </button>
          </div>
        </aside>

        <Show when={!isChannelRailCollapsed()}>
          <aside class="channel-rail">
            <header class="channel-rail-header">
              <h2>{activeWorkspace()?.guildName ?? "No Workspace"}</h2>
              <button
                type="button"
                class="channel-rail-header-action"
                aria-label="Open settings from channel rail"
                title="Settings"
                onClick={() => openOverlayPanel("settings")}
              >
                *
              </button>
            </header>
            <span class="channel-rail-subtitle">
              {activeWorkspace() ? `${activeWorkspace()!.visibility} workspace` : "Hardened workspace"}
            </span>

            <Switch>
              <Match when={!activeWorkspace()}>
                <p class="muted">Create a workspace to begin.</p>
              </Match>
              <Match when={activeWorkspace()}>
                <nav aria-label="channels" class="channel-nav">
                  <section class="channel-group">
                    <div class="channel-group-header">
                      <p class="group-label">TEXT CHANNELS</p>
                      <Show when={canManageWorkspaceChannels()}>
                        <button
                          type="button"
                          class="channel-group-action"
                          aria-label="Create text channel"
                          title="Create text channel"
                          onClick={() => {
                            setNewChannelKind(channelKindFromInput("text"));
                            openOverlayPanel("channel-create");
                          }}
                        >
                          +
                        </button>
                      </Show>
                    </div>
                    <For each={activeTextChannels()}>
                      {(channel) => (
                        <button
                          classList={{
                            active: activeChannelId() === channel.channelId,
                            "channel-row": true,
                          }}
                          aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                          onClick={() => setActiveChannelId(channel.channelId)}
                        >
                          <span class="channel-row-main">
                            <span class="channel-row-kind" aria-hidden="true">
                              #
                            </span>
                            <span>{channel.name}</span>
                          </span>
                        </button>
                      )}
                    </For>
                  </section>

                  <section class="channel-group">
                    <div class="channel-group-header">
                      <p class="group-label">VOICE CHANNELS</p>
                      <Show when={canManageWorkspaceChannels()}>
                        <button
                          type="button"
                          class="channel-group-action"
                          aria-label="Create voice channel"
                          title="Create voice channel"
                          onClick={() => {
                            setNewChannelKind(channelKindFromInput("voice"));
                            openOverlayPanel("channel-create");
                          }}
                        >
                          +
                        </button>
                      </Show>
                    </div>
                    <For each={activeVoiceChannels()}>
                      {(channel) => (
                        <div class="voice-channel-entry">
                          <button
                            classList={{
                              active: activeChannelId() === channel.channelId,
                              "channel-row": true,
                            }}
                            aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                            onClick={() => setActiveChannelId(channel.channelId)}
                          >
                            <span class="channel-row-main">
                              <span class="channel-row-kind channel-row-kind-voice" aria-hidden="true">
                                VC
                              </span>
                              <span>{channel.name}</span>
                            </span>
                            <Show when={isVoiceSessionForChannel(channel.channelId)}>
                              <span class="channel-row-status">{voiceSessionDurationLabel()}</span>
                            </Show>
                          </button>
                          <Show when={isVoiceSessionForChannel(channel.channelId)}>
                            <section class="voice-channel-presence" aria-label="In-call participants">
                              <Show
                                when={voiceRosterEntries().length > 0}
                                fallback={
                                  <p class="voice-channel-presence-empty">Waiting for participants...</p>
                                }
                              >
                                <ul class="voice-channel-presence-tree">
                                  <For each={voiceRosterEntries()}>
                                    {(entry) => (
                                      <li
                                        classList={{
                                          "voice-channel-presence-participant": true,
                                          "voice-channel-presence-participant-local": entry.isLocal,
                                          "voice-channel-presence-participant-speaking": entry.isSpeaking,
                                        }}
                                      >
                                        <span class="voice-tree-avatar" aria-hidden="true">
                                          {actorAvatarGlyph(actorLabel(entry.identity))}
                                        </span>
                                        <span
                                          classList={{
                                            "voice-channel-presence-name": true,
                                            "voice-channel-presence-name-speaking": entry.isSpeaking,
                                          }}
                                        >
                                          {voiceParticipantLabel(entry.identity, entry.isLocal)}
                                        </span>
                                        <span class="voice-channel-presence-badges">
                                          <Show when={entry.hasCamera}>
                                            <span class="voice-participant-media-badge video">Video</span>
                                          </Show>
                                          <Show when={entry.hasScreenShare}>
                                            <span class="voice-participant-media-badge screen">Share</span>
                                          </Show>
                                        </span>
                                      </li>
                                    )}
                                  </For>
                                </ul>
                              </Show>
                              <Show when={voiceStreamPermissionHints().length > 0}>
                                <div
                                  class="voice-channel-stream-hints"
                                  aria-label="Voice stream permission status"
                                >
                                  <For each={voiceStreamPermissionHints()}>
                                    {(hint) => <p>{hint}</p>}
                                  </For>
                                </div>
                              </Show>
                            </section>
                          </Show>
                        </div>
                      )}
                    </For>
                  </section>
                </nav>
                <Show when={canShowVoiceHeaderControls() || isVoiceSessionActive()}>
                  <section class="voice-connected-dock" aria-label="Voice connected dock">
                    <div class="voice-connected-dock-head">
                      <p class="voice-connected-dock-title">
                        {isVoiceSessionActive() ? "Voice Connected" : "Voice Channel Ready"}
                      </p>
                      <Show when={isVoiceSessionActive()}>
                        <span class="voice-connected-dock-duration">{voiceSessionDurationLabel()}</span>
                      </Show>
                    </div>
                    <p class="voice-connected-dock-channel">
                      {isVoiceSessionActive()
                        ? activeVoiceSessionLabel()
                        : channelHeaderLabel({ kind: activeChannel()!.kind, name: activeChannel()!.name })}
                    </p>
                    <div class="voice-connected-dock-controls">
                      <Show when={canShowVoiceHeaderControls() && !isVoiceSessionActive()}>
                        <button
                          type="button"
                          onClick={() => void joinVoiceChannel()}
                          disabled={isJoiningVoice() || isLeavingVoice()}
                        >
                          {isJoiningVoice() ? "Joining..." : "Join Voice"}
                        </button>
                      </Show>
                      <Show when={isVoiceSessionActive()}>
                        <button
                          type="button"
                          onClick={() => void toggleVoiceMicrophone()}
                          disabled={
                            isTogglingVoiceMic() ||
                            rtcSnapshot().connectionStatus !== "connected" ||
                            isJoiningVoice() ||
                            isLeavingVoice()
                          }
                        >
                          {isTogglingVoiceMic()
                            ? "Updating..."
                            : rtcSnapshot().isMicrophoneEnabled
                              ? "Mute Mic"
                              : "Unmute Mic"}
                        </button>
                        <button
                          type="button"
                          onClick={() => void toggleVoiceCamera()}
                          disabled={
                            isTogglingVoiceCamera() ||
                            rtcSnapshot().connectionStatus !== "connected" ||
                            isJoiningVoice() ||
                            isLeavingVoice() ||
                            !canToggleVoiceCamera()
                          }
                        >
                          {isTogglingVoiceCamera()
                            ? "Updating..."
                            : rtcSnapshot().isCameraEnabled
                              ? "Camera Off"
                              : "Camera On"}
                        </button>
                        <button
                          type="button"
                          onClick={() => void toggleVoiceScreenShare()}
                          disabled={
                            isTogglingVoiceScreenShare() ||
                            rtcSnapshot().connectionStatus !== "connected" ||
                            isJoiningVoice() ||
                            isLeavingVoice() ||
                            !canToggleVoiceScreenShare()
                          }
                        >
                          {isTogglingVoiceScreenShare()
                            ? "Updating..."
                            : rtcSnapshot().isScreenShareEnabled
                              ? "Stop Share"
                              : "Share Screen"}
                        </button>
                        <button
                          type="button"
                          onClick={() => void leaveVoiceChannel("Voice session ended.")}
                          disabled={isLeavingVoice() || isJoiningVoice()}
                        >
                          {isLeavingVoice() ? "Leaving..." : "Leave"}
                        </button>
                      </Show>
                    </div>
                  </section>
                </Show>
              </Match>
            </Switch>
          </aside>
        </Show>

        <main class="chat-panel">
        <header class="chat-header">
          <div>
            <h3>
              {activeChannel()
                ? channelHeaderLabel({ kind: activeChannel()!.kind, name: activeChannel()!.name })
                : "#no-channel"}
            </h3>
            <p>Gateway {gatewayOnline() ? "connected" : "disconnected"}</p>
          </div>
          <div class="header-actions">
            <span classList={{ "gateway-badge": true, online: gatewayOnline() }}>
              {gatewayOnline() ? "Live" : "Offline"}
            </span>
            <Show when={canShowVoiceHeaderControls() || isVoiceSessionActive()}>
              <span
                classList={{
                  "voice-badge": true,
                  connected: voiceConnectionState() === "connected",
                  connecting: voiceConnectionState() === "connecting",
                  reconnecting: voiceConnectionState() === "reconnecting",
                  error: voiceConnectionState() === "error",
                }}
              >
                Voice {voiceConnectionState()}
              </span>
            </Show>
            <button type="button" onClick={() => setChannelRailCollapsed((value) => !value)}>
              {isChannelRailCollapsed() ? "Show channels" : "Hide channels"}
            </button>
            <button type="button" onClick={() => setMemberRailCollapsed((value) => !value)}>
              {isMemberRailCollapsed() ? "Show members" : "Hide members"}
            </button>
            <button type="button" onClick={() => openOverlayPanel("public-directory")}>
              Directory
            </button>
            <button type="button" onClick={() => openOverlayPanel("friendships")}>
              Friends
            </button>
            <button type="button" onClick={() => void refreshMessages()}>
              Refresh
            </button>
            <button type="button" onClick={() => void refreshSession()} disabled={isRefreshingSession()}>
              {isRefreshingSession() ? "Refreshing..." : "Refresh session"}
            </button>
            <button class="logout" onClick={() => void logout()}>
              Logout
            </button>
          </div>
        </header>

        <Show
          when={workspaceBootstrapDone() && workspaces().length === 0}
          fallback={
            <>
              <Show when={!workspaceBootstrapDone()}>
                <p class="panel-note">Validating workspace access...</p>
              </Show>
              <Show when={workspaceBootstrapDone()}>
                <Show when={isLoadingMessages()}>
                  <p class="panel-note">Loading messages...</p>
                </Show>
                <Show when={messageError()}>
                  <p class="status error panel-note">{messageError()}</p>
                </Show>
                <Show when={sessionStatus()}>
                  <p class="status ok panel-note">{sessionStatus()}</p>
                </Show>
                <Show when={sessionError()}>
                  <p class="status error panel-note">{sessionError()}</p>
                </Show>
                <Show when={voiceStatus() && (canShowVoiceHeaderControls() || isVoiceSessionActive())}>
                  <p class="status ok panel-note">{voiceStatus()}</p>
                </Show>
                <Show when={voiceError() && (canShowVoiceHeaderControls() || isVoiceSessionActive())}>
                  <p class="status error panel-note">{voiceError()}</p>
                </Show>
                <Show when={activeChannel() && !canAccessActiveChannel()}>
                  <p class="status error panel-note">
                    Channel is not visible with your current default permissions.
                  </p>
                </Show>

                <section class="message-list" aria-live="polite">
                  <Show when={nextBefore()}>
                    <button type="button" class="load-older" onClick={() => void loadOlderMessages()} disabled={isLoadingOlder()}>
                      {isLoadingOlder() ? "Loading older..." : "Load older messages"}
                    </button>
                  </Show>

                  <For each={messages()}>
                    {(message) => {
                      const reactions = createMemo(() =>
                        reactionViewsForMessage(
                          message.messageId,
                          reactionState(),
                          pendingReactionByKey(),
                        ),
                      );
                      const isEditing = () => editingMessageId() === message.messageId;
                      const isDeleting = () => deletingMessageId() === message.messageId;
                      const isReactionPickerOpen =
                        () => openReactionPickerMessageId() === message.messageId;
                      const canEditOrDelete =
                        () => profile()?.userId === message.authorId || canDeleteMessages();
                      return (
                        <article class="message-row">
                          <div class="message-avatar" aria-hidden="true">
                            {actorAvatarGlyph(displayUserLabel(message.authorId))}
                          </div>
                          <div class="message-main">
                            <p class="message-meta">
                              <strong>{displayUserLabel(message.authorId)}</strong>
                              <span>{formatMessageTime(message.createdAtUnix)}</span>
                            </p>
                            <Show
                              when={isEditing()}
                              fallback={
                                <Show when={tokenizeToDisplayText(message.markdownTokens) || message.content}>
                                  <p class="message-tokenized">
                                    {tokenizeToDisplayText(message.markdownTokens) || message.content}
                                  </p>
                                </Show>
                              }
                            >
                              <form
                                class="inline-form message-edit"
                                onSubmit={(event) => {
                                  event.preventDefault();
                                  void saveEditMessage(message.messageId);
                                }}
                              >
                                <input
                                  value={editingDraft()}
                                  onInput={(event) => setEditingDraft(event.currentTarget.value)}
                                  maxlength="2000"
                                />
                                <div class="message-actions">
                                  <button type="submit" disabled={isSavingEdit()}>
                                    {isSavingEdit() ? "Saving..." : "Save"}
                                  </button>
                                  <button type="button" onClick={cancelEditMessage}>
                                    Cancel
                                  </button>
                                </div>
                              </form>
                            </Show>
                            <Show when={message.attachments.length > 0}>
                              <div class="message-attachments">
                                <For each={message.attachments}>
                                  {(record) => {
                                    const preview = () => messageMediaByAttachmentId()[record.attachmentId];
                                    return (
                                      <div class="message-attachment-card">
                                        <Show
                                          when={preview() && preview()!.kind === "image"}
                                          fallback={
                                            <Show
                                              when={preview() && preview()!.kind === "video"}
                                              fallback={
                                                <Show
                                                  when={loadingMediaPreviewIds()[record.attachmentId]}
                                                  fallback={
                                                    <Show
                                                      when={failedMediaPreviewIds()[record.attachmentId]}
                                                      fallback={
                                                        <button
                                                          type="button"
                                                          class="message-attachment-download"
                                                          onClick={() => void downloadAttachment(record)}
                                                          disabled={downloadingAttachmentId() === record.attachmentId}
                                                        >
                                                          {downloadingAttachmentId() === record.attachmentId
                                                            ? "Fetching..."
                                                            : `Download ${record.filename}`}
                                                        </button>
                                                      }
                                                    >
                                                      <div class="message-attachment-failed">
                                                        <span>Preview unavailable.</span>
                                                        <button
                                                          type="button"
                                                          class="message-attachment-retry"
                                                          onClick={() => retryMediaPreview(record.attachmentId)}
                                                        >
                                                          Retry preview
                                                        </button>
                                                      </div>
                                                    </Show>
                                                  }
                                                >
                                                  <p class="message-attachment-loading">Loading preview...</p>
                                                </Show>
                                              }
                                            >
                                              <video
                                                class="message-attachment-video"
                                                src={preview()!.url}
                                                controls
                                                preload="metadata"
                                                playsinline
                                              />
                                            </Show>
                                          }
                                        >
                                          <img
                                            class="message-attachment-image"
                                            src={preview()!.url}
                                            alt={record.filename}
                                            loading="lazy"
                                            decoding="async"
                                            referrerPolicy="no-referrer"
                                          />
                                        </Show>
                                        <p class="message-attachment-meta">
                                          {record.filename} ({formatBytes(record.sizeBytes)})
                                        </p>
                                      </div>
                                    );
                                  }}
                                </For>
                              </div>
                            </Show>
                            <Show when={canEditOrDelete()}>
                              <div class="message-actions compact">
                                <button
                                  type="button"
                                  class="icon-button"
                                  onClick={() => beginEditMessage(message)}
                                  aria-label="Edit message"
                                  title="Edit message"
                                >
                                  <span
                                    class="icon-mask"
                                    style={`--icon-url: url("${EDIT_MESSAGE_ICON_URL}")`}
                                    aria-hidden="true"
                                  />
                                </button>
                                <button
                                  type="button"
                                  classList={{ "icon-button": true, "is-busy": isDeleting(), danger: true }}
                                  onClick={() => void removeMessage(message.messageId)}
                                  disabled={isDeleting()}
                                  aria-label="Delete message"
                                  title={isDeleting() ? "Deleting message..." : "Delete message"}
                                >
                                  <span
                                    class="icon-mask"
                                    style={`--icon-url: url("${DELETE_MESSAGE_ICON_URL}")`}
                                    aria-hidden="true"
                                  />
                                </button>
                              </div>
                            </Show>
                            <div class="reaction-row">
                              <div class="reaction-controls">
                                <div class="reaction-list">
                                  <For each={reactions()}>
                                    {(reaction) => (
                                      <button
                                        type="button"
                                        classList={{ "reaction-chip": true, reacted: reaction.reacted }}
                                        onClick={() =>
                                          void toggleMessageReaction(message.messageId, reaction.emoji)}
                                        disabled={reaction.pending}
                                        aria-label={`${reaction.emoji} reaction (${reaction.count})`}
                                      >
                                        <span class="reaction-chip-emoji">{reaction.emoji}</span>
                                        <span class="reaction-chip-count">{reaction.count}</span>
                                      </button>
                                    )}
                                  </For>
                                </div>
                                <button
                                  type="button"
                                  class="icon-button reaction-add-trigger"
                                  onClick={() => toggleReactionPicker(message.messageId)}
                                  aria-label="Add reaction"
                                  title="Add reaction"
                                >
                                  <span
                                    class="icon-mask"
                                    style={`--icon-url: url("${ADD_REACTION_ICON_URL}")`}
                                    aria-hidden="true"
                                  />
                                </button>
                              </div>
                              <Show when={isReactionPickerOpen()}>
                                <div class="reaction-picker" role="dialog" aria-label="Choose reaction">
                                  <div class="reaction-picker-header">
                                    <p class="reaction-picker-title">React</p>
                                    <button
                                      type="button"
                                      class="reaction-picker-close"
                                      onClick={() => setOpenReactionPickerMessageId(null)}
                                    >
                                      Close
                                    </button>
                                  </div>
                                  <div class="reaction-picker-grid">
                                    <For each={OPENMOJI_REACTION_OPTIONS}>
                                      {(option) => (
                                        <button
                                          type="button"
                                          class="reaction-picker-option"
                                          onClick={() =>
                                            void addReactionFromPicker(message.messageId, option.emoji)}
                                          aria-label={`Add ${option.label} reaction`}
                                          title={option.label}
                                        >
                                          <img
                                            src={option.iconUrl}
                                            alt=""
                                            loading="lazy"
                                            decoding="async"
                                            referrerPolicy="no-referrer"
                                          />
                                        </button>
                                      )}
                                    </For>
                                  </div>
                                </div>
                              </Show>
                            </div>
                          </div>
                        </article>
                      );
                    }}
                  </For>

                  <Show when={!isLoadingMessages() && messages().length === 0 && !messageError()}>
                    <p class="muted">No messages yet in this channel.</p>
                  </Show>
                </section>

                <form class="composer" onSubmit={sendMessage}>
                  <input
                    ref={(value) => {
                      composerAttachmentInputRef = value;
                    }}
                    type="file"
                    multiple
                    class="composer-file-input"
                    onInput={onComposerAttachmentInput}
                  />
                  <div class="composer-input-shell">
                    <button
                      type="button"
                      class="composer-attach-button"
                      onClick={openComposerAttachmentPicker}
                      disabled={!activeChannel() || isSendingMessage() || !canAccessActiveChannel()}
                      aria-label="Attach files"
                      title="Attach files"
                    >
                      +
                    </button>
                    <input
                      class="composer-text-input"
                      value={composer()}
                      onInput={(event) => setComposer(event.currentTarget.value)}
                      maxlength="2000"
                      placeholder={
                        activeChannel()
                          ? `Message ${channelRailLabel({ kind: activeChannel()!.kind, name: activeChannel()!.name })}`
                          : "Select channel"
                      }
                      disabled={!activeChannel() || isSendingMessage() || !canAccessActiveChannel()}
                    />
                    <button
                      type="submit"
                      class="composer-send-button"
                      disabled={!activeChannel() || isSendingMessage() || !canAccessActiveChannel()}
                    >
                      {isSendingMessage() ? "Sending..." : "Send"}
                    </button>
                  </div>
                  <Show when={composerAttachments().length > 0}>
                    <div class="composer-attachments">
                      <For each={composerAttachments()}>
                        {(file) => (
                          <button
                            type="button"
                            class="composer-attachment-pill"
                            onClick={() => removeComposerAttachment(file)}
                            title={`Remove ${file.name}`}
                          >
                            {file.name} ({formatBytes(file.size)}) x
                          </button>
                        )}
                      </For>
                    </div>
                  </Show>
                </form>
              </Show>
            </>
          }
        >
          <section class="empty-workspace">
            <h3>Create your first workspace</h3>
            <p class="muted">Use the + button in the workspace rail to create your first guild and channel.</p>
          </section>
        </Show>

        <Show when={messageStatus()}>
          <p class="status ok panel-note">{messageStatus()}</p>
        </Show>
      </main>

        <Show when={!isMemberRailCollapsed()}>
          <aside class="member-rail">
            <header>
              <h4>Workspace Tools</h4>
            </header>

            <Show when={profile.loading}>
              <p class="muted">Loading profile...</p>
            </Show>
            <Show when={profile.error}>
              <p class="status error">{profileErrorMessage(profile.error)}</p>
            </Show>
            <Show when={profile()}>
              {(value) => (
                <div class="profile-card">
                  <p class="label">Username</p>
                  <p>{value().username}</p>
                  <p class="label">User ID</p>
                  <p class="mono">{value().userId}</p>
                </div>
              )}
            </Show>

            <Show when={activeWorkspace() && activeChannel() && !canAccessActiveChannel()}>
              <p class="muted">No authorized workspace/channel selected for operator actions.</p>
            </Show>

            <Show when={canAccessActiveChannel()}>
              <section class="member-group">
                <p class="group-label">ONLINE ({onlineMembers().length})</p>
                <ul>
                  <For each={onlineMembers()}>
                    {(memberId) => (
                      <li>
                        <span class="presence online" />
                        {displayUserLabel(memberId)}
                      </li>
                    )}
                  </For>
                  <Show when={onlineMembers().length === 0}>
                    <li>
                      <span class="presence idle" />
                      no-presence-yet
                    </li>
                  </Show>
                </ul>
              </section>
            </Show>

            <section class="member-group">
              <p class="group-label">PANELS</p>
              <div class="ops-launch-grid">
                <button type="button" onClick={() => openOverlayPanel("public-directory")}>
                  Open directory panel
                </button>
                <button type="button" onClick={() => openOverlayPanel("friendships")}>
                  Open friendships panel
                </button>
                <Show when={canAccessActiveChannel()}>
                  <button type="button" onClick={() => openOverlayPanel("search")}>
                    Open search panel
                  </button>
                  <button type="button" onClick={() => openOverlayPanel("attachments")}>
                    Open attachments panel
                  </button>
                </Show>
                <Show when={hasModerationAccess()}>
                  <button type="button" onClick={() => openOverlayPanel("moderation")}>
                    Open moderation panel
                  </button>
                </Show>
                <button type="button" onClick={() => openOverlayPanel("utility")}>
                  Open utility panel
                </button>
              </div>
            </section>
          </aside>
        </Show>
      </div>

      <Show when={activeOverlayPanel()}>
        {(panel) => (
          <div
            class="panel-backdrop"
            role="presentation"
            onClick={(event) => {
              if (event.target === event.currentTarget) {
                closeOverlayPanel();
              }
            }}
          >
            <section
              class={overlayPanelClassName(panel())}
              role="dialog"
              aria-modal="true"
              aria-label={`${overlayPanelTitle(panel())} panel`}
            >
              <header class="panel-window-header">
                <h4>{overlayPanelTitle(panel())}</h4>
                <button type="button" onClick={closeOverlayPanel} disabled={!canCloseActivePanel()}>
                  Close
                </button>
              </header>
              <div class="panel-window-body">
                <Switch>
                  <Match when={panel() === "workspace-create"}>
                    <section class="member-group">
                      <form class="inline-form" onSubmit={createWorkspace}>
                        <label>
                          Workspace name
                          <input
                            value={createGuildName()}
                            onInput={(event) => setCreateGuildName(event.currentTarget.value)}
                            maxlength="64"
                          />
                        </label>
                        <label>
                          Visibility
                          <select
                            value={createGuildVisibility()}
                            onChange={(event) =>
                              setCreateGuildVisibility(guildVisibilityFromInput(event.currentTarget.value))
                            }
                          >
                            <option value="private">private</option>
                            <option value="public">public</option>
                          </select>
                        </label>
                        <label>
                          First channel
                          <input
                            value={createChannelName()}
                            onInput={(event) => setCreateChannelName(event.currentTarget.value)}
                            maxlength="64"
                          />
                        </label>
                        <label>
                          Channel type
                          <select
                            value={createChannelKind()}
                            onChange={(event) =>
                              setCreateChannelKind(channelKindFromInput(event.currentTarget.value))
                            }
                          >
                            <option value="text">text</option>
                            <option value="voice">voice</option>
                          </select>
                        </label>
                        <div class="button-row">
                          <button type="submit" disabled={isCreatingWorkspace()}>
                            {isCreatingWorkspace() ? "Creating..." : "Create workspace"}
                          </button>
                          <Show when={canDismissWorkspaceCreateForm()}>
                            <button type="button" onClick={closeOverlayPanel}>
                              Cancel
                            </button>
                          </Show>
                        </div>
                      </form>
                      <Show when={workspaceError()}>
                        <p class="status error">{workspaceError()}</p>
                      </Show>
                    </section>
                  </Match>

                  <Match when={panel() === "channel-create" && canManageWorkspaceChannels()}>
                    <section class="member-group">
                      <form class="inline-form" onSubmit={createNewChannel}>
                        <label>
                          Channel name
                          <input
                            value={newChannelName()}
                            onInput={(event) => setNewChannelName(event.currentTarget.value)}
                            maxlength="64"
                          />
                        </label>
                        <label>
                          Channel type
                          <select
                            value={newChannelKind()}
                            onChange={(event) =>
                              setNewChannelKind(channelKindFromInput(event.currentTarget.value))
                            }
                          >
                            <option value="text">text</option>
                            <option value="voice">voice</option>
                          </select>
                        </label>
                        <div class="button-row">
                          <button type="submit" disabled={isCreatingChannel()}>
                            {isCreatingChannel() ? "Creating..." : "Create channel"}
                          </button>
                          <button type="button" onClick={closeOverlayPanel}>
                            Cancel
                          </button>
                        </div>
                      </form>
                      <Show when={channelCreateError()}>
                        <p class="status error">{channelCreateError()}</p>
                      </Show>
                    </section>
                  </Match>

                  <Match when={panel() === "public-directory"}>
                    <section class="public-directory" aria-label="public-workspace-directory">
                      <form class="inline-form" onSubmit={runPublicGuildSearch}>
                        <label>
                          Search
                          <input
                            value={publicGuildSearchQuery()}
                            onInput={(event) => setPublicGuildSearchQuery(event.currentTarget.value)}
                            maxlength="64"
                            placeholder="workspace name"
                          />
                        </label>
                        <button type="submit" disabled={isSearchingPublicGuilds()}>
                          {isSearchingPublicGuilds() ? "Searching..." : "Find public"}
                        </button>
                      </form>
                      <Show when={publicGuildSearchError()}>
                        <p class="status error">{publicGuildSearchError()}</p>
                      </Show>
                      <ul>
                        <For each={publicGuildDirectory()}>
                          {(guild) => (
                            <li>
                              <span class="presence online" />
                              <div class="stacked-meta">
                                <span>{guild.name}</span>
                                <span class="muted mono">{guild.visibility}</span>
                              </div>
                            </li>
                          )}
                        </For>
                        <Show when={!isSearchingPublicGuilds() && publicGuildDirectory().length === 0}>
                          <li>
                            <span class="presence idle" />
                            no-public-workspaces
                          </li>
                        </Show>
                      </ul>
                    </section>
                  </Match>

                  <Match when={panel() === "settings"}>
                    <section class="settings-panel-layout" aria-label="settings">
                      <aside class="settings-panel-rail" aria-label="Settings category rail">
                        <p class="group-label">CATEGORIES</p>
                        <ul class="settings-category-list">
                          <For each={SETTINGS_CATEGORIES}>
                            {(category) => {
                              const isActive = () => activeSettingsCategory() === category.id;
                              return (
                                <li>
                                  <button
                                    type="button"
                                    class="settings-category-button"
                                    classList={{ "settings-category-button-active": isActive() }}
                                    onClick={() => openSettingsCategory(category.id)}
                                    aria-label={`Open ${category.label} settings category`}
                                    aria-current={isActive() ? "page" : undefined}
                                  >
                                    <span class="settings-category-name">{category.label}</span>
                                    <span class="settings-category-summary muted">{category.summary}</span>
                                  </button>
                                </li>
                              );
                            }}
                          </For>
                        </ul>
                      </aside>
                      <section class="settings-panel-content" aria-label="Settings content pane">
                        <Switch>
                          <Match when={activeSettingsCategory() === "voice"}>
                            <section class="settings-submenu-layout" aria-label="Voice settings submenu">
                              <aside class="settings-submenu-rail" aria-label="Voice settings submenu rail">
                                <p class="group-label">VOICE</p>
                                <ul class="settings-submenu-list">
                                  <For each={VOICE_SETTINGS_SUBMENU}>
                                    {(submenu) => {
                                      const isActive = () => activeVoiceSettingsSubmenu() === submenu.id;
                                      return (
                                        <li>
                                          <button
                                            type="button"
                                            class="settings-submenu-button"
                                            classList={{
                                              "settings-submenu-button-active": isActive(),
                                            }}
                                            onClick={() => setActiveVoiceSettingsSubmenu(submenu.id)}
                                            aria-label={`Open Voice ${submenu.label} submenu`}
                                            aria-current={isActive() ? "page" : undefined}
                                          >
                                            <span class="settings-category-name">{submenu.label}</span>
                                            <span class="settings-category-summary muted">
                                              {submenu.summary}
                                            </span>
                                          </button>
                                        </li>
                                      );
                                    }}
                                  </For>
                                </ul>
                              </aside>
                              <section
                                class="settings-submenu-content"
                                aria-label="Voice settings submenu content"
                              >
                                <Switch>
                                  <Match when={activeVoiceSettingsSubmenu() === "audio-devices"}>
                                    <p class="group-label">AUDIO DEVICES</p>
                                    <form class="inline-form" onSubmit={(event) => event.preventDefault()}>
                                      <label>
                                        Microphone
                                        <select
                                          aria-label="Select microphone device"
                                          value={voiceDevicePreferences().audioInputDeviceId ?? ""}
                                          onChange={(event) =>
                                            void setVoiceDevicePreference(
                                              "audioinput",
                                              event.currentTarget.value,
                                            )
                                          }
                                          disabled={isRefreshingAudioDevices()}
                                        >
                                          <option value="">System default</option>
                                          <For each={audioInputDevices()}>
                                            {(device) => (
                                              <option value={device.deviceId}>{device.label}</option>
                                            )}
                                          </For>
                                        </select>
                                      </label>
                                      <label>
                                        Speaker
                                        <select
                                          aria-label="Select speaker device"
                                          value={voiceDevicePreferences().audioOutputDeviceId ?? ""}
                                          onChange={(event) =>
                                            void setVoiceDevicePreference(
                                              "audiooutput",
                                              event.currentTarget.value,
                                            )
                                          }
                                          disabled={isRefreshingAudioDevices()}
                                        >
                                          <option value="">System default</option>
                                          <For each={audioOutputDevices()}>
                                            {(device) => (
                                              <option value={device.deviceId}>{device.label}</option>
                                            )}
                                          </For>
                                        </select>
                                      </label>
                                      <button
                                        type="button"
                                        onClick={() => void refreshAudioDeviceInventory()}
                                        disabled={isRefreshingAudioDevices()}
                                      >
                                        {isRefreshingAudioDevices() ? "Refreshing..." : "Refresh devices"}
                                      </button>
                                    </form>
                                    <Show when={audioDevicesStatus()}>
                                      <p class="status ok">{audioDevicesStatus()}</p>
                                    </Show>
                                    <Show when={audioDevicesError()}>
                                      <p class="status error">{audioDevicesError()}</p>
                                    </Show>
                                    <Show
                                      when={
                                        !isRefreshingAudioDevices() &&
                                        audioInputDevices().length === 0 &&
                                        audioOutputDevices().length === 0 &&
                                        !audioDevicesError()
                                      }
                                    >
                                      <p class="muted">
                                        No audio devices were detected yet. Refresh after granting
                                        media permissions.
                                      </p>
                                    </Show>
                                  </Match>
                                </Switch>
                              </section>
                            </section>
                          </Match>
                          <Match when={activeSettingsCategory() === "profile"}>
                            <p class="group-label">PROFILE</p>
                            <p class="muted">
                              Profile settings remain a non-functional placeholder for a future
                              plan phase.
                            </p>
                          </Match>
                        </Switch>
                      </section>
                    </section>
                  </Match>

                  <Match when={panel() === "friendships"}>
                    <section class="public-directory" aria-label="friendships">
                      <form class="inline-form" onSubmit={submitFriendRequest}>
                        <label>
                          User ID
                          <input
                            value={friendRecipientUserIdInput()}
                            onInput={(event) => setFriendRecipientUserIdInput(event.currentTarget.value)}
                            maxlength="26"
                            placeholder="01ARZ3NDEKTSV4RRFFQ69G5FAV"
                          />
                        </label>
                        <button type="submit" disabled={isRunningFriendAction()}>
                          {isRunningFriendAction() ? "Submitting..." : "Send request"}
                        </button>
                      </form>
                      <Show when={friendStatus()}>
                        <p class="status ok">{friendStatus()}</p>
                      </Show>
                      <Show when={friendError()}>
                        <p class="status error">{friendError()}</p>
                      </Show>

                      <p class="group-label">INCOMING</p>
                      <ul>
                        <For each={friendRequests().incoming}>
                          {(request) => (
                            <li>
                              <div class="stacked-meta">
                                <span>{request.senderUsername}</span>
                                <span class="muted mono">{request.senderUserId}</span>
                              </div>
                              <div class="button-row">
                                <button
                                  type="button"
                                  onClick={() => void acceptIncomingFriendRequest(request.requestId)}
                                  disabled={isRunningFriendAction()}
                                >
                                  Accept
                                </button>
                                <button
                                  type="button"
                                  onClick={() => void dismissFriendRequest(request.requestId)}
                                  disabled={isRunningFriendAction()}
                                >
                                  Ignore
                                </button>
                              </div>
                            </li>
                          )}
                        </For>
                        <Show when={friendRequests().incoming.length === 0}>
                          <li class="muted">no-incoming-requests</li>
                        </Show>
                      </ul>

                      <p class="group-label">OUTGOING</p>
                      <ul>
                        <For each={friendRequests().outgoing}>
                          {(request) => (
                            <li>
                              <div class="stacked-meta">
                                <span>{request.recipientUsername}</span>
                                <span class="muted mono">{request.recipientUserId}</span>
                              </div>
                              <button
                                type="button"
                                onClick={() => void dismissFriendRequest(request.requestId)}
                                disabled={isRunningFriendAction()}
                              >
                                Cancel
                              </button>
                            </li>
                          )}
                        </For>
                        <Show when={friendRequests().outgoing.length === 0}>
                          <li class="muted">no-outgoing-requests</li>
                        </Show>
                      </ul>

                      <p class="group-label">FRIEND LIST</p>
                      <ul>
                        <For each={friends()}>
                          {(friend) => (
                            <li>
                              <div class="stacked-meta">
                                <span>{friend.username}</span>
                                <span class="muted mono">{friend.userId}</span>
                              </div>
                              <button
                                type="button"
                                onClick={() => void removeFriendship(friend.userId)}
                                disabled={isRunningFriendAction()}
                              >
                                Remove
                              </button>
                            </li>
                          )}
                        </For>
                        <Show when={friends().length === 0}>
                          <li class="muted">no-friends</li>
                        </Show>
                      </ul>
                    </section>
                  </Match>

                  <Match when={panel() === "search" && canAccessActiveChannel()}>
                    <section class="member-group">
                      <form class="inline-form" onSubmit={runSearch}>
                        <label>
                          Query
                          <input
                            value={searchQuery()}
                            onInput={(event) => setSearchQuery(event.currentTarget.value)}
                            maxlength="256"
                            placeholder="needle"
                          />
                        </label>
                        <button type="submit" disabled={isSearching() || !activeWorkspace()}>
                          {isSearching() ? "Searching..." : "Search"}
                        </button>
                      </form>
                      <Show when={canManageSearchMaintenance()}>
                        <div class="button-row">
                          <button
                            type="button"
                            onClick={() => void rebuildSearch()}
                            disabled={isRunningSearchOps() || !activeWorkspace()}
                          >
                            Rebuild Index
                          </button>
                          <button
                            type="button"
                            onClick={() => void reconcileSearch()}
                            disabled={isRunningSearchOps() || !activeWorkspace()}
                          >
                            Reconcile Index
                          </button>
                        </div>
                      </Show>
                      <Show when={searchOpsStatus()}>
                        <p class="status ok">{searchOpsStatus()}</p>
                      </Show>
                      <Show when={searchError()}>
                        <p class="status error">{searchError()}</p>
                      </Show>
                      <Show when={searchResults()}>
                        {(results) => (
                          <ul>
                            <For each={results().messages}>
                              {(message) => (
                                <li>
                                  <span class="presence online" />
                                  {displayUserLabel(message.authorId)}:{" "}
                                  {(tokenizeToDisplayText(message.markdownTokens) || message.content).slice(0, 40)}
                                </li>
                              )}
                            </For>
                          </ul>
                        )}
                      </Show>
                    </section>
                  </Match>

                  <Match when={panel() === "attachments" && canAccessActiveChannel()}>
                    <section class="member-group">
                      <form class="inline-form" onSubmit={uploadAttachment}>
                        <label>
                          File
                          <input
                            type="file"
                            onInput={(event) => {
                              const file = event.currentTarget.files?.[0] ?? null;
                              setSelectedAttachment(file);
                              setAttachmentFilename(file?.name ?? "");
                            }}
                          />
                        </label>
                        <label>
                          Filename
                          <input
                            value={attachmentFilename()}
                            onInput={(event) => setAttachmentFilename(event.currentTarget.value)}
                            maxlength="128"
                            placeholder="upload.bin"
                          />
                        </label>
                        <button type="submit" disabled={isUploadingAttachment() || !activeChannel()}>
                          {isUploadingAttachment() ? "Uploading..." : "Upload"}
                        </button>
                      </form>
                      <Show when={attachmentStatus()}>
                        <p class="status ok">{attachmentStatus()}</p>
                      </Show>
                      <Show when={attachmentError()}>
                        <p class="status error">{attachmentError()}</p>
                      </Show>
                      <ul>
                        <For each={activeAttachments()}>
                          {(record) => (
                            <li>
                              <span class="presence online" />
                              <div class="stacked-meta">
                                <span>{record.filename}</span>
                                <span class="muted mono">
                                  {record.mimeType} · {formatBytes(record.sizeBytes)}
                                </span>
                              </div>
                              <button
                                type="button"
                                onClick={() => void downloadAttachment(record)}
                                disabled={downloadingAttachmentId() === record.attachmentId}
                              >
                                {downloadingAttachmentId() === record.attachmentId ? "..." : "Get"}
                              </button>
                              <button
                                type="button"
                                onClick={() => void removeAttachment(record)}
                                disabled={deletingAttachmentId() === record.attachmentId}
                              >
                                {deletingAttachmentId() === record.attachmentId ? "..." : "Del"}
                              </button>
                            </li>
                          )}
                        </For>
                        <Show when={activeAttachments().length === 0}>
                          <li>
                            <span class="presence idle" />
                            no-local-attachments
                          </li>
                        </Show>
                      </ul>
                    </section>
                  </Match>

                  <Match when={panel() === "moderation" && hasModerationAccess()}>
                    <section class="member-group">
                      <form class="inline-form">
                        <label>
                          Target user ULID
                          <input
                            value={moderationUserIdInput()}
                            onInput={(event) => setModerationUserIdInput(event.currentTarget.value)}
                            maxlength="26"
                            placeholder="01ARZ..."
                          />
                        </label>
                        <label>
                          Role
                          <select
                            value={moderationRoleInput()}
                            onChange={(event) => setModerationRoleInput(roleFromInput(event.currentTarget.value))}
                          >
                            <option value="member">member</option>
                            <option value="moderator">moderator</option>
                            <option value="owner">owner</option>
                          </select>
                        </label>
                        <div class="button-row">
                          <Show when={canManageRoles()}>
                            <button
                              type="button"
                              disabled={isModerating() || !activeWorkspace()}
                              onClick={() => void runMemberAction("add")}
                            >
                              Add
                            </button>
                            <button
                              type="button"
                              disabled={isModerating() || !activeWorkspace()}
                              onClick={() => void runMemberAction("role")}
                            >
                              Set Role
                            </button>
                          </Show>
                          <Show when={canBanMembers()}>
                            <button
                              type="button"
                              disabled={isModerating() || !activeWorkspace()}
                              onClick={() => void runMemberAction("kick")}
                            >
                              Kick
                            </button>
                            <button
                              type="button"
                              disabled={isModerating() || !activeWorkspace()}
                              onClick={() => void runMemberAction("ban")}
                            >
                              Ban
                            </button>
                          </Show>
                        </div>
                      </form>
                      <Show when={canManageChannelOverrides()}>
                        <form class="inline-form" onSubmit={applyOverride}>
                          <label>
                            Override role
                            <select
                              value={overrideRoleInput()}
                              onChange={(event) => setOverrideRoleInput(roleFromInput(event.currentTarget.value))}
                            >
                              <option value="member">member</option>
                              <option value="moderator">moderator</option>
                              <option value="owner">owner</option>
                            </select>
                          </label>
                          <label>
                            Allow permissions (csv)
                            <input
                              value={overrideAllowCsv()}
                              onInput={(event) => setOverrideAllowCsv(event.currentTarget.value)}
                              placeholder="create_message,subscribe_streams"
                            />
                          </label>
                          <label>
                            Deny permissions (csv)
                            <input
                              value={overrideDenyCsv()}
                              onInput={(event) => setOverrideDenyCsv(event.currentTarget.value)}
                              placeholder="delete_message"
                            />
                          </label>
                          <button type="submit" disabled={isModerating() || !activeChannel()}>
                            Apply channel override
                          </button>
                        </form>
                      </Show>
                      <Show when={moderationStatus()}>
                        <p class="status ok">{moderationStatus()}</p>
                      </Show>
                      <Show when={moderationError()}>
                        <p class="status error">{moderationError()}</p>
                      </Show>
                    </section>
                  </Match>

                  <Match when={panel() === "utility"}>
                    <section class="member-group">
                      <div class="button-row">
                        <button type="button" onClick={() => void runHealthCheck()} disabled={isCheckingHealth()}>
                          {isCheckingHealth() ? "Checking..." : "Health"}
                        </button>
                      </div>
                      <form class="inline-form" onSubmit={runEcho}>
                        <label>
                          Echo
                          <input
                            value={echoInput()}
                            onInput={(event) => setEchoInput(event.currentTarget.value)}
                            maxlength="128"
                          />
                        </label>
                        <button type="submit" disabled={isEchoing()}>
                          {isEchoing() ? "Sending..." : "Echo"}
                        </button>
                      </form>
                      <Show when={healthStatus()}>
                        <p class="status ok">{healthStatus()}</p>
                      </Show>
                      <Show when={diagError()}>
                        <p class="status error">{diagError()}</p>
                      </Show>
                    </section>
                  </Match>
                </Switch>
              </div>
            </section>
          </div>
        )}
      </Show>
    </div>
  );
}
