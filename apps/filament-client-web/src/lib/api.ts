import {
  type AuthSession,
  type Password,
  type Username,
  authSessionFromResponse,
  type AccessToken,
  type CaptchaToken,
  type RefreshToken,
} from "../domain/auth";
import {
  type AttachmentFilename,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelId,
  type ChannelKindName,
  type ChannelRecord,
  type ChannelPermissionSnapshot,
  type ChannelName,
  type GuildRecord,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type FriendRecord,
  type FriendRequestCreateResult,
  type FriendRequestList,
  type MediaPublishSource,
  type MessageContent,
  type MessageHistory,
  type MessageId,
  type MessageRecord,
  type PublicGuildDirectory,
  type ModerationResult,
  type PermissionName,
  type ReactionEmoji,
  type ReactionRecord,
  type RoleName,
  type SearchQuery,
  type SearchReconcileResult,
  type SearchResults,
  type UserLookupRecord,
  type UserId,
  type VoiceTokenRecord,
  attachmentFromResponse,
  channelFromResponse,
  channelPermissionSnapshotFromResponse,
  friendListFromResponse,
  friendRequestCreateFromResponse,
  friendRequestListFromResponse,
  guildFromResponse,
  messageFromResponse,
  messageHistoryFromResponse,
  moderationResultFromResponse,
  publicGuildDirectoryFromResponse,
  reactionFromResponse,
  searchReconcileFromResponse,
  searchResultsFromResponse,
  userLookupListFromResponse,
  userIdFromInput,
  voiceTokenFromResponse,
} from "../domain/chat";
import { bearerHeader } from "./session";

const DEFAULT_API_ORIGIN = "https://api.filament.local";
const MAX_RESPONSE_BYTES = 64 * 1024;
const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;
const MAX_ATTACHMENT_DOWNLOAD_BYTES = 26 * 1024 * 1024;
const REQUEST_TIMEOUT_MS = 7_000;

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

interface ApiConfig {
  baseUrl: string;
}

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
  headers?: Record<string, string>;
}

interface BodyRequest {
  method: "POST" | "PATCH" | "DELETE";
  path: string;
  body: BodyInit;
  accessToken?: AccessToken;
  headers?: Record<string, string>;
}

function resolvedBaseUrl(): string {
  const envValue = import.meta.env.VITE_FILAMENT_API_BASE_URL;
  if (typeof envValue === "string" && envValue.length > 0) {
    return envValue;
  }
  if (import.meta.env.DEV) {
    return "/api";
  }
  return DEFAULT_API_ORIGIN;
}

function apiConfig(): ApiConfig {
  return {
    baseUrl: resolvedBaseUrl(),
  };
}

async function fetchWithTimeout(
  input: RequestInfo | URL,
  init: RequestInit,
  timeoutMs: number,
): Promise<Response> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort("timeout"), timeoutMs);

  try {
    return await fetch(input, {
      ...init,
      signal: controller.signal,
    });
  } finally {
    window.clearTimeout(timeout);
  }
}

async function sendRequest(input: {
  method: string;
  path: string;
  accessToken?: AccessToken;
  headers?: Record<string, string>;
  body?: BodyInit;
  timeoutMs?: number;
}): Promise<Response> {
  const config = apiConfig();
  const headers: Record<string, string> = { ...(input.headers ?? {}) };
  if (input.accessToken) {
    headers.authorization = bearerHeader(input.accessToken);
  }

  try {
    return await fetchWithTimeout(
      `${config.baseUrl}${input.path}`,
      {
        method: input.method,
        headers,
        body: input.body,
        credentials: "omit",
      },
      input.timeoutMs ?? REQUEST_TIMEOUT_MS,
    );
  } catch {
    throw new ApiError(0, "network_error", "Unable to reach Filament API.");
  }
}

async function readBoundedJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text.length > MAX_RESPONSE_BYTES) {
    throw new ApiError(response.status, "oversized_response", "Response too large.");
  }

  if (text.length === 0) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    throw new ApiError(response.status, "invalid_json", "Malformed server response.");
  }
}

function readApiError(data: unknown): string {
  if (!data || typeof data !== "object") {
    return "unexpected_error";
  }
  const error = (data as { error?: unknown }).error;
  return typeof error === "string" ? error : "unexpected_error";
}

async function requestJson(request: JsonRequest): Promise<unknown> {
  const headers: Record<string, string> = { ...(request.headers ?? {}) };
  if (request.body !== undefined) {
    headers["content-type"] = "application/json";
  }
  const response = await sendRequest({
    method: request.method,
    path: request.path,
    accessToken: request.accessToken,
    headers,
    body: request.body === undefined ? undefined : JSON.stringify(request.body),
  });

  const data = await readBoundedJson(response);
  if (!response.ok) {
    const code = readApiError(data);
    throw new ApiError(response.status, code, code);
  }
  return data;
}

async function requestNoContent(request: JsonRequest): Promise<void> {
  const dto = await requestJson(request);
  if (dto !== null) {
    throw new ApiError(500, "unexpected_body", "Expected empty response body.");
  }
}

async function requestJsonWithBody(request: BodyRequest): Promise<unknown> {
  const response = await sendRequest({
    method: request.method,
    path: request.path,
    accessToken: request.accessToken,
    headers: request.headers,
    body: request.body,
  });
  const data = await readBoundedJson(response);
  if (!response.ok) {
    throw new ApiError(response.status, readApiError(data), readApiError(data));
  }
  return data;
}

async function requestBinary(input: {
  path: string;
  accessToken: AccessToken;
  timeoutMs?: number;
  maxBytes?: number;
}): Promise<{ bytes: Uint8Array; mimeType: string | null }> {
  const response = await sendRequest({
    method: "GET",
    path: input.path,
    accessToken: input.accessToken,
    timeoutMs: input.timeoutMs,
  });
  if (!response.ok) {
    const data = await readBoundedJson(response);
    throw new ApiError(response.status, readApiError(data), readApiError(data));
  }
  const arrayBuffer = await response.arrayBuffer();
  if (arrayBuffer.byteLength > (input.maxBytes ?? MAX_ATTACHMENT_DOWNLOAD_BYTES)) {
    throw new ApiError(response.status, "oversized_response", "Attachment response too large.");
  }
  return {
    bytes: new Uint8Array(arrayBuffer),
    mimeType: response.headers.get("content-type"),
  };
}

export async function registerWithPassword(input: {
  username: Username;
  password: Password;
  captchaToken?: CaptchaToken;
}): Promise<void> {
  const dto = await requestJson({
    method: "POST",
    path: "/auth/register",
    body: {
      username: input.username,
      password: input.password,
      ...(input.captchaToken ? { captcha_token: input.captchaToken } : {}),
    },
  });

  if (!dto || typeof dto !== "object" || (dto as { accepted?: unknown }).accepted !== true) {
    throw new ApiError(500, "invalid_register_shape", "Unexpected register response.");
  }
}

export async function loginWithPassword(input: {
  username: Username;
  password: Password;
}): Promise<AuthSession> {
  const dto = await requestJson({
    method: "POST",
    path: "/auth/login",
    body: {
      username: input.username,
      password: input.password,
    },
  });

  if (!dto || typeof dto !== "object") {
    throw new ApiError(500, "invalid_login_shape", "Unexpected login response.");
  }

  return authSessionFromResponse(dto as {
    access_token: string;
    refresh_token: string;
    expires_in_secs: number;
  });
}

export async function refreshAuthSession(
  refreshToken: RefreshToken,
): Promise<AuthSession> {
  const dto = await requestJson({
    method: "POST",
    path: "/auth/refresh",
    body: { refresh_token: refreshToken },
  });

  if (!dto || typeof dto !== "object") {
    throw new ApiError(500, "invalid_refresh_shape", "Unexpected refresh response.");
  }
  return authSessionFromResponse(dto as {
    access_token: string;
    refresh_token: string;
    expires_in_secs: number;
  });
}

export async function logoutAuthSession(refreshToken: RefreshToken): Promise<void> {
  await requestNoContent({
    method: "POST",
    path: "/auth/logout",
    body: { refresh_token: refreshToken },
  });
}

export async function fetchMe(session: AuthSession): Promise<{ userId: UserId; username: string }> {
  const dto = await requestJson({
    method: "GET",
    path: "/auth/me",
    accessToken: session.accessToken,
  });

  if (!dto || typeof dto !== "object") {
    throw new ApiError(500, "invalid_me_shape", "Unexpected profile response.");
  }

  const userId = (dto as { user_id?: unknown }).user_id;
  const username = (dto as { username?: unknown }).username;
  if (typeof userId !== "string" || typeof username !== "string") {
    throw new ApiError(500, "invalid_me_shape", "Unexpected profile response.");
  }

  return {
    userId: userIdFromInput(userId),
    username,
  };
}

export async function lookupUsersByIds(
  session: AuthSession,
  userIds: UserId[],
): Promise<UserLookupRecord[]> {
  if (userIds.length < 1 || userIds.length > 64) {
    throw new ApiError(400, "invalid_request", "user_ids must contain 1-64 values.");
  }
  const dto = await requestJson({
    method: "POST",
    path: "/users/lookup",
    accessToken: session.accessToken,
    body: { user_ids: userIds },
  });
  return userLookupListFromResponse(dto);
}

export async function fetchFriends(session: AuthSession): Promise<FriendRecord[]> {
  const dto = await requestJson({
    method: "GET",
    path: "/friends",
    accessToken: session.accessToken,
  });
  return friendListFromResponse(dto);
}

export async function fetchFriendRequests(session: AuthSession): Promise<FriendRequestList> {
  const dto = await requestJson({
    method: "GET",
    path: "/friends/requests",
    accessToken: session.accessToken,
  });
  return friendRequestListFromResponse(dto);
}

export async function createFriendRequest(
  session: AuthSession,
  recipientUserId: UserId,
): Promise<FriendRequestCreateResult> {
  const dto = await requestJson({
    method: "POST",
    path: "/friends/requests",
    accessToken: session.accessToken,
    body: { recipient_user_id: recipientUserId },
  });
  return friendRequestCreateFromResponse(dto);
}

export async function acceptFriendRequest(
  session: AuthSession,
  requestId: string,
): Promise<void> {
  const dto = await requestJson({
    method: "POST",
    path: `/friends/requests/${requestId}/accept`,
    accessToken: session.accessToken,
  });
  if (!dto || typeof dto !== "object" || (dto as { accepted?: unknown }).accepted !== true) {
    throw new ApiError(500, "invalid_friend_accept_shape", "Unexpected friend accept response.");
  }
}

export async function deleteFriendRequest(
  session: AuthSession,
  requestId: string,
): Promise<void> {
  await requestNoContent({
    method: "DELETE",
    path: `/friends/requests/${requestId}`,
    accessToken: session.accessToken,
  });
}

export async function removeFriend(
  session: AuthSession,
  friendUserId: UserId,
): Promise<void> {
  await requestNoContent({
    method: "DELETE",
    path: `/friends/${friendUserId}`,
    accessToken: session.accessToken,
  });
}

export async function fetchHealth(): Promise<{ status: "ok" }> {
  const dto = await requestJson({
    method: "GET",
    path: "/health",
  });
  if (!dto || typeof dto !== "object" || (dto as { status?: unknown }).status !== "ok") {
    throw new ApiError(500, "invalid_health_shape", "Unexpected health response.");
  }
  return { status: "ok" };
}

export async function echoMessage(input: { message: string }): Promise<string> {
  const dto = await requestJson({
    method: "POST",
    path: "/echo",
    body: { message: input.message },
  });
  if (!dto || typeof dto !== "object" || typeof (dto as { message?: unknown }).message !== "string") {
    throw new ApiError(500, "invalid_echo_shape", "Unexpected echo response.");
  }
  return (dto as { message: string }).message;
}

export async function createGuild(
  session: AuthSession,
  input: { name: GuildName; visibility?: GuildVisibility },
): Promise<{
  guildId: GuildId;
  name: GuildName;
  visibility: GuildVisibility;
}> {
  const dto = await requestJson({
    method: "POST",
    path: "/guilds",
    accessToken: session.accessToken,
    body: { name: input.name, visibility: input.visibility },
  });
  return guildFromResponse(dto);
}

export async function fetchGuilds(session: AuthSession): Promise<GuildRecord[]> {
  const dto = await requestJson({
    method: "GET",
    path: "/guilds",
    accessToken: session.accessToken,
  });
  if (!dto || typeof dto !== "object" || !Array.isArray((dto as { guilds?: unknown }).guilds)) {
    throw new ApiError(500, "invalid_guild_list_shape", "Unexpected guild list response.");
  }
  return (dto as { guilds: unknown[] }).guilds.map((entry) => guildFromResponse(entry));
}

export async function fetchPublicGuildDirectory(
  session: AuthSession,
  input?: { query?: string; limit?: number },
): Promise<PublicGuildDirectory> {
  const params = new URLSearchParams();
  const query = input?.query?.trim();
  if (query && query.length > 0) {
    params.set("q", query.slice(0, 64));
  }
  if (input?.limit && Number.isInteger(input.limit) && input.limit > 0 && input.limit <= 50) {
    params.set("limit", String(input.limit));
  }
  const suffix = params.size > 0 ? `?${params.toString()}` : "";
  const dto = await requestJson({
    method: "GET",
    path: `/guilds/public${suffix}`,
    accessToken: session.accessToken,
  });
  return publicGuildDirectoryFromResponse(dto);
}

export async function fetchGuildChannels(
  session: AuthSession,
  guildId: GuildId,
): Promise<ChannelRecord[]> {
  const dto = await requestJson({
    method: "GET",
    path: `/guilds/${guildId}/channels`,
    accessToken: session.accessToken,
  });
  if (!dto || typeof dto !== "object" || !Array.isArray((dto as { channels?: unknown }).channels)) {
    throw new ApiError(500, "invalid_channel_list_shape", "Unexpected channel list response.");
  }
  return (dto as { channels: unknown[] }).channels.map((entry) => channelFromResponse(entry));
}

export async function createChannel(
  session: AuthSession,
  guildId: GuildId,
  input: { name: ChannelName; kind: ChannelKindName },
): Promise<ChannelRecord> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels`,
    accessToken: session.accessToken,
    body: { name: input.name, kind: input.kind },
  });
  return channelFromResponse(dto);
}

export async function fetchChannelPermissionSnapshot(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
): Promise<ChannelPermissionSnapshot> {
  const dto = await requestJson({
    method: "GET",
    path: `/guilds/${guildId}/channels/${channelId}/permissions/self`,
    accessToken: session.accessToken,
  });
  return channelPermissionSnapshotFromResponse(dto);
}

export async function fetchChannelMessages(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  input?: { limit?: number; before?: MessageId },
): Promise<MessageHistory> {
  const params = new URLSearchParams();
  if (input?.limit && Number.isInteger(input.limit) && input.limit > 0 && input.limit <= 100) {
    params.set("limit", String(input.limit));
  }
  if (input?.before) {
    params.set("before", input.before);
  }

  const query = params.size > 0 ? `?${params.toString()}` : "";
  const dto = await requestJson({
    method: "GET",
    path: `/guilds/${guildId}/channels/${channelId}/messages${query}`,
    accessToken: session.accessToken,
  });
  return messageHistoryFromResponse(dto);
}

export async function createChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  input: { content: MessageContent; attachmentIds?: AttachmentId[] },
): Promise<MessageRecord> {
  try {
    const dto = await requestJson({
      method: "POST",
      path: `/guilds/${guildId}/channels/${channelId}/messages`,
      accessToken: session.accessToken,
      body: {
        content: input.content,
        attachment_ids: input.attachmentIds,
      },
    });
    return messageFromResponse(dto);
  } catch (error) {
    if (
      error instanceof ApiError &&
      error.code === "invalid_json" &&
      input.attachmentIds &&
      input.attachmentIds.length > 0
    ) {
      throw new ApiError(
        400,
        "protocol_mismatch",
        "Server does not support attachment_ids on message create.",
      );
    }
    throw error;
  }
}

export async function editChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
  input: { content: MessageContent },
): Promise<MessageRecord> {
  const dto = await requestJson({
    method: "PATCH",
    path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}`,
    accessToken: session.accessToken,
    body: { content: input.content },
  });
  return messageFromResponse(dto);
}

export async function deleteChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
): Promise<void> {
  await requestNoContent({
    method: "DELETE",
    path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}`,
    accessToken: session.accessToken,
  });
}

export async function searchGuildMessages(
  session: AuthSession,
  guildId: GuildId,
  input: { query: SearchQuery; limit?: number; channelId?: ChannelId },
): Promise<SearchResults> {
  const params = new URLSearchParams();
  params.set("q", input.query);
  if (input.limit && Number.isInteger(input.limit) && input.limit > 0 && input.limit <= 50) {
    params.set("limit", String(input.limit));
  }
  if (input.channelId) {
    params.set("channel_id", input.channelId);
  }

  const dto = await requestJson({
    method: "GET",
    path: `/guilds/${guildId}/search?${params.toString()}`,
    accessToken: session.accessToken,
  });
  return searchResultsFromResponse(dto);
}

export async function rebuildGuildSearchIndex(
  session: AuthSession,
  guildId: GuildId,
): Promise<void> {
  await requestNoContent({
    method: "POST",
    path: `/guilds/${guildId}/search/rebuild`,
    accessToken: session.accessToken,
  });
}

export async function reconcileGuildSearchIndex(
  session: AuthSession,
  guildId: GuildId,
): Promise<SearchReconcileResult> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/search/reconcile`,
    accessToken: session.accessToken,
  });
  return searchReconcileFromResponse(dto);
}

export async function addMessageReaction(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
  emoji: ReactionEmoji,
): Promise<ReactionRecord> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
    accessToken: session.accessToken,
  });
  return reactionFromResponse(dto);
}

export async function removeMessageReaction(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
  emoji: ReactionEmoji,
): Promise<ReactionRecord> {
  const dto = await requestJson({
    method: "DELETE",
    path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
    accessToken: session.accessToken,
  });
  return reactionFromResponse(dto);
}

export async function uploadChannelAttachment(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  file: File,
  filename: AttachmentFilename,
): Promise<AttachmentRecord> {
  if (file.size < 1 || file.size > MAX_ATTACHMENT_BYTES) {
    throw new ApiError(400, "invalid_request", "Attachment size must be within server limits.");
  }
  const query = new URLSearchParams({ filename });
  const headers: Record<string, string> = {};
  if (file.type && file.type.length <= 128) {
    headers["content-type"] = file.type;
  }
  const dto = await requestJsonWithBody({
    method: "POST",
    path: `/guilds/${guildId}/channels/${channelId}/attachments?${query.toString()}`,
    accessToken: session.accessToken,
    headers,
    body: file,
  });
  return attachmentFromResponse(dto);
}

export async function downloadChannelAttachment(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  attachmentId: AttachmentId,
): Promise<{ bytes: Uint8Array; mimeType: string | null }> {
  return requestBinary({
    path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
    accessToken: session.accessToken,
  });
}

export async function downloadChannelAttachmentPreview(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  attachmentId: AttachmentId,
): Promise<{ bytes: Uint8Array; mimeType: string | null }> {
  return requestBinary({
    path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
    accessToken: session.accessToken,
    timeoutMs: 2_500,
    maxBytes: 12 * 1024 * 1024,
  });
}

export async function deleteChannelAttachment(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  attachmentId: AttachmentId,
): Promise<void> {
  await requestNoContent({
    method: "DELETE",
    path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
    accessToken: session.accessToken,
  });
}

export async function addGuildMember(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
): Promise<ModerationResult> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/members/${userId}`,
    accessToken: session.accessToken,
  });
  return moderationResultFromResponse(dto);
}

export async function updateGuildMemberRole(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
  role: RoleName,
): Promise<ModerationResult> {
  const dto = await requestJson({
    method: "PATCH",
    path: `/guilds/${guildId}/members/${userId}`,
    accessToken: session.accessToken,
    body: { role },
  });
  return moderationResultFromResponse(dto);
}

export async function kickGuildMember(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
): Promise<ModerationResult> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/members/${userId}/kick`,
    accessToken: session.accessToken,
  });
  return moderationResultFromResponse(dto);
}

export async function banGuildMember(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
): Promise<ModerationResult> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/members/${userId}/ban`,
    accessToken: session.accessToken,
  });
  return moderationResultFromResponse(dto);
}

export async function setChannelRoleOverride(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  role: RoleName,
  input: {
    allow: PermissionName[];
    deny: PermissionName[];
  },
): Promise<ModerationResult> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels/${channelId}/overrides/${role}`,
    accessToken: session.accessToken,
    body: input,
  });
  return moderationResultFromResponse(dto);
}

export async function issueVoiceToken(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  input: {
    canPublish?: boolean;
    canSubscribe?: boolean;
    publishSources?: MediaPublishSource[];
  },
): Promise<VoiceTokenRecord> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels/${channelId}/voice/token`,
    accessToken: session.accessToken,
    body: {
      can_publish: input.canPublish,
      can_subscribe: input.canSubscribe,
      publish_sources: input.publishSources,
    },
  });
  return voiceTokenFromResponse(dto);
}
