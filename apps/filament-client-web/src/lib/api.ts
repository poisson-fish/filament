import {
  type AuthSession,
  type Password,
  type Username,
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
  type ChannelName,
  type ChannelPermissionSnapshot,
  type ChannelRecord,
  type DirectoryJoinResult,
  type GuildRecord,
  type GuildId,
  type GuildRoleList,
  type GuildRoleRecord,
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
  type ProfileRecord,
  type WorkspaceRoleId,
  type WorkspaceRoleName,
  type UserLookupRecord,
  type UserId,
  type VoiceTokenRecord,
  attachmentFromResponse,
  profileFromResponse,
  searchReconcileFromResponse,
  searchResultsFromResponse,
  userLookupListFromResponse,
} from "../domain/chat";
import { bearerHeader } from "./session";
import { createAuthApi } from "./api-auth";
import { createFriendsApi } from "./api-friends";
import { createMessagesApi } from "./api-messages";
import { createVoiceApi } from "./api-voice";
import { createWorkspaceApi } from "./api-workspace";

const DEFAULT_API_ORIGIN = "https://api.filament.local";
const MAX_RESPONSE_BYTES = 64 * 1024;
const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;
const MAX_ATTACHMENT_DOWNLOAD_BYTES = 26 * 1024 * 1024;
const MAX_PROFILE_AVATAR_BYTES = 2 * 1024 * 1024;
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

function parseContentLength(headers: Headers): number | null {
  const rawValue = headers.get("content-length");
  if (!rawValue) {
    return null;
  }
  const trimmedValue = rawValue.trim();
  if (!/^\d+$/.test(trimmedValue)) {
    return null;
  }
  const parsed = Number.parseInt(trimmedValue, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    return null;
  }
  return parsed;
}

async function cancelResponseBody(body: ReadableStream<Uint8Array> | null): Promise<void> {
  if (!body) {
    return;
  }
  try {
    await body.cancel("response_limit_exceeded");
  } catch {
    // Ignore cancellation errors: limit violation is the primary failure.
  }
}

async function readBoundedResponseBytes(input: {
  response: Response;
  maxBytes: number;
  oversizedError: ApiError;
  timeoutMs?: number;
}): Promise<Uint8Array> {
  const contentLength = parseContentLength(input.response.headers);
  if (contentLength !== null && contentLength > input.maxBytes) {
    await cancelResponseBody(input.response.body);
    throw input.oversizedError;
  }

  const body = input.response.body;
  if (!body) {
    return new Uint8Array();
  }

  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  let timeoutId: number | undefined;

  try {
    const timeoutMs = input.timeoutMs ?? 30_000;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutId = window.setTimeout(() => {
        reject(
          new ApiError(
            input.response.status,
            "request_timeout",
            "Response read timed out.",
          ),
        );
      }, timeoutMs);
    });

    while (true) {
      const { done, value } = await Promise.race([
        reader.read(),
        timeoutPromise,
      ]);

      if (done) {
        break;
      }
      if (!value || value.byteLength === 0) {
        continue;
      }
      totalBytes += value.byteLength;
      if (totalBytes > input.maxBytes) {
        try {
          await reader.cancel("response_limit_exceeded");
        } catch {
          // Ignore cancellation errors: limit violation is the primary failure.
        }
        throw input.oversizedError;
      }
      chunks.push(value);
    }
  } catch (error) {
    try {
      await reader.cancel("read_error");
    } catch {
      // Ignore cancellation errors
    }
    throw error;
  } finally {
    if (timeoutId !== undefined) {
      window.clearTimeout(timeoutId);
    }
    reader.releaseLock();
  }

  const bytes = new Uint8Array(totalBytes);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

async function readBoundedJson(response: Response): Promise<unknown> {
  const bytes = await readBoundedResponseBytes({
    response,
    maxBytes: MAX_RESPONSE_BYTES,
    oversizedError: new ApiError(response.status, "oversized_response", "Response too large."),
  });
  if (bytes.byteLength === 0) {
    return null;
  }
  const text = new TextDecoder().decode(bytes);

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

function fallbackErrorCodeFromStatus(status: number): string {
  if (status === 400) {
    return "invalid_request";
  }
  if (status === 401) {
    return "invalid_credentials";
  }
  if (status === 403) {
    return "forbidden";
  }
  if (status === 404) {
    return "not_found";
  }
  if (status === 408) {
    return "request_timeout";
  }
  if (status === 413) {
    return "payload_too_large";
  }
  if (status === 429) {
    return "rate_limited";
  }
  if (status >= 500) {
    return "internal_error";
  }
  return "unexpected_error";
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

  let data: unknown;
  try {
    data = await readBoundedJson(response);
  } catch (error) {
    if (
      error instanceof ApiError &&
      error.code === "invalid_json" &&
      !response.ok
    ) {
      const code = fallbackErrorCodeFromStatus(response.status);
      throw new ApiError(response.status, code, code);
    }
    throw error;
  }
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
  let data: unknown;
  try {
    data = await readBoundedJson(response);
  } catch (error) {
    if (
      error instanceof ApiError &&
      error.code === "invalid_json" &&
      !response.ok
    ) {
      const code = fallbackErrorCodeFromStatus(response.status);
      throw new ApiError(response.status, code, code);
    }
    throw error;
  }
  if (!response.ok) {
    const code = readApiError(data);
    throw new ApiError(response.status, code, code);
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
    let data: unknown;
    try {
      data = await readBoundedJson(response);
    } catch (error) {
      if (error instanceof ApiError && error.code === "invalid_json") {
        const code = fallbackErrorCodeFromStatus(response.status);
        throw new ApiError(response.status, code, code);
      }
      throw error;
    }
    const code = readApiError(data);
    throw new ApiError(response.status, code, code);
  }
  const bytes = await readBoundedResponseBytes({
    response,
    maxBytes: input.maxBytes ?? MAX_ATTACHMENT_DOWNLOAD_BYTES,
    oversizedError: new ApiError(
      response.status,
      "oversized_response",
      "Attachment response too large.",
    ),
    timeoutMs: input.timeoutMs,
  });
  return {
    bytes,
    mimeType: response.headers.get("content-type"),
  };
}

const authApi = createAuthApi({
  requestJson,
  requestNoContent,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

const friendsApi = createFriendsApi({
  requestJson,
  requestNoContent,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

const messagesApi = createMessagesApi({
  requestJson,
  requestNoContent,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
  isApiErrorCode(error, code) {
    return error instanceof ApiError && error.code === code;
  },
});

const voiceApi = createVoiceApi({
  requestJson,
});

const workspaceApi = createWorkspaceApi({
  requestJson,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

export async function registerWithPassword(input: {
  username: Username;
  password: Password;
  captchaToken?: CaptchaToken;
}): Promise<void> {
  await authApi.registerWithPassword(input);
}

export async function loginWithPassword(input: {
  username: Username;
  password: Password;
}): Promise<AuthSession> {
  return authApi.loginWithPassword(input);
}

export async function refreshAuthSession(
  refreshToken: RefreshToken,
): Promise<AuthSession> {
  return authApi.refreshAuthSession(refreshToken);
}

export async function logoutAuthSession(refreshToken: RefreshToken): Promise<void> {
  await authApi.logoutAuthSession(refreshToken);
}

export async function fetchMe(session: AuthSession): Promise<ProfileRecord> {
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
  const data = dto as {
    about_markdown?: unknown;
    about_markdown_tokens?: unknown;
    avatar_version?: unknown;
  };
  return profileFromResponse({
    user_id: userId,
    username,
    about_markdown: typeof data.about_markdown === "string" ? data.about_markdown : "",
    about_markdown_tokens: Array.isArray(data.about_markdown_tokens)
      ? data.about_markdown_tokens
      : [],
    avatar_version: Number.isInteger(data.avatar_version) ? data.avatar_version : 0,
  });
}

export async function fetchUserProfile(
  session: AuthSession,
  userId: UserId,
): Promise<ProfileRecord> {
  const dto = await requestJson({
    method: "GET",
    path: `/users/${userId}/profile`,
    accessToken: session.accessToken,
  });
  return profileFromResponse(dto);
}

export async function updateMyProfile(
  session: AuthSession,
  input: { username?: Username; aboutMarkdown?: string },
): Promise<ProfileRecord> {
  const body: Record<string, unknown> = {};
  if (input.username) {
    body.username = input.username;
  }
  if (typeof input.aboutMarkdown === "string") {
    body.about_markdown = input.aboutMarkdown;
  }
  const dto = await requestJson({
    method: "PATCH",
    path: "/users/me/profile",
    accessToken: session.accessToken,
    body,
  });
  return profileFromResponse(dto);
}

export async function uploadMyProfileAvatar(
  session: AuthSession,
  file: File,
): Promise<ProfileRecord> {
  if (file.size < 1 || file.size > MAX_PROFILE_AVATAR_BYTES) {
    throw new ApiError(
      400,
      "invalid_request",
      "Avatar size must be within server limits.",
    );
  }
  const headers: Record<string, string> = {};
  if (file.type && file.type.length <= 128) {
    headers["content-type"] = file.type;
  }
  const dto = await requestJsonWithBody({
    method: "POST",
    path: "/users/me/profile/avatar",
    accessToken: session.accessToken,
    headers,
    body: file,
  });
  return profileFromResponse(dto);
}

export function profileAvatarUrl(userId: UserId, avatarVersion: number): string {
  const config = apiConfig();
  return `${config.baseUrl}/users/${userId}/avatar?v=${Math.max(0, Math.trunc(avatarVersion))}`;
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
  return friendsApi.fetchFriends(session);
}

export async function fetchFriendRequests(session: AuthSession): Promise<FriendRequestList> {
  return friendsApi.fetchFriendRequests(session);
}

export async function createFriendRequest(
  session: AuthSession,
  recipientUserId: UserId,
): Promise<FriendRequestCreateResult> {
  return friendsApi.createFriendRequest(session, recipientUserId);
}

export async function acceptFriendRequest(
  session: AuthSession,
  requestId: string,
): Promise<void> {
  await friendsApi.acceptFriendRequest(session, requestId);
}

export async function deleteFriendRequest(
  session: AuthSession,
  requestId: string,
): Promise<void> {
  await friendsApi.deleteFriendRequest(session, requestId);
}

export async function removeFriend(
  session: AuthSession,
  friendUserId: UserId,
): Promise<void> {
  await friendsApi.removeFriend(session, friendUserId);
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
  return workspaceApi.createGuild(session, input);
}

export async function fetchGuilds(session: AuthSession): Promise<GuildRecord[]> {
  return workspaceApi.fetchGuilds(session);
}

export async function updateGuild(
  session: AuthSession,
  guildId: GuildId,
  input: { name: GuildName; visibility?: GuildVisibility },
): Promise<GuildRecord> {
  return workspaceApi.updateGuild(session, guildId, input);
}

export async function fetchPublicGuildDirectory(
  session: AuthSession,
  input?: { query?: string; limit?: number },
): Promise<PublicGuildDirectory> {
  return workspaceApi.fetchPublicGuildDirectory(session, input);
}

export async function joinPublicGuild(
  session: AuthSession,
  guildId: GuildId,
): Promise<DirectoryJoinResult> {
  return workspaceApi.joinPublicGuild(session, guildId);
}

export async function fetchGuildChannels(
  session: AuthSession,
  guildId: GuildId,
): Promise<ChannelRecord[]> {
  return workspaceApi.fetchGuildChannels(session, guildId);
}

export async function createChannel(
  session: AuthSession,
  guildId: GuildId,
  input: { name: ChannelName; kind: ChannelKindName },
): Promise<ChannelRecord> {
  return workspaceApi.createChannel(session, guildId, input);
}

export async function fetchChannelPermissionSnapshot(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
): Promise<ChannelPermissionSnapshot> {
  return workspaceApi.fetchChannelPermissionSnapshot(session, guildId, channelId);
}

export async function fetchChannelMessages(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  input?: { limit?: number; before?: MessageId },
): Promise<MessageHistory> {
  return messagesApi.fetchChannelMessages(session, guildId, channelId, input);
}

export async function createChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  input: { content: MessageContent; attachmentIds?: AttachmentId[] },
): Promise<MessageRecord> {
  return messagesApi.createChannelMessage(session, guildId, channelId, input);
}

export async function editChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
  input: { content: MessageContent },
): Promise<MessageRecord> {
  return messagesApi.editChannelMessage(session, guildId, channelId, messageId, input);
}

export async function deleteChannelMessage(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
): Promise<void> {
  await messagesApi.deleteChannelMessage(session, guildId, channelId, messageId);
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
  return messagesApi.addMessageReaction(session, guildId, channelId, messageId, emoji);
}

export async function removeMessageReaction(
  session: AuthSession,
  guildId: GuildId,
  channelId: ChannelId,
  messageId: MessageId,
  emoji: ReactionEmoji,
): Promise<ReactionRecord> {
  return messagesApi.removeMessageReaction(session, guildId, channelId, messageId, emoji);
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
    timeoutMs: 15_000,
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
  return workspaceApi.addGuildMember(session, guildId, userId);
}

export async function updateGuildMemberRole(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
  role: RoleName,
): Promise<ModerationResult> {
  return workspaceApi.updateGuildMemberRole(session, guildId, userId, role);
}

export async function kickGuildMember(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
): Promise<ModerationResult> {
  return workspaceApi.kickGuildMember(session, guildId, userId);
}

export async function banGuildMember(
  session: AuthSession,
  guildId: GuildId,
  userId: UserId,
): Promise<ModerationResult> {
  return workspaceApi.banGuildMember(session, guildId, userId);
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
  return workspaceApi.setChannelRoleOverride(session, guildId, channelId, role, input);
}

export async function fetchGuildRoles(
  session: AuthSession,
  guildId: GuildId,
): Promise<GuildRoleList> {
  return workspaceApi.fetchGuildRoles(session, guildId);
}

export async function createGuildRole(
  session: AuthSession,
  guildId: GuildId,
  input: {
    name: WorkspaceRoleName;
    permissions: PermissionName[];
    position?: number;
  },
): Promise<GuildRoleRecord> {
  return workspaceApi.createGuildRole(session, guildId, input);
}

export async function updateGuildRole(
  session: AuthSession,
  guildId: GuildId,
  roleId: WorkspaceRoleId,
  input: {
    name?: WorkspaceRoleName;
    permissions?: PermissionName[];
  },
): Promise<GuildRoleRecord> {
  return workspaceApi.updateGuildRole(session, guildId, roleId, input);
}

export async function deleteGuildRole(
  session: AuthSession,
  guildId: GuildId,
  roleId: WorkspaceRoleId,
): Promise<ModerationResult> {
  return workspaceApi.deleteGuildRole(session, guildId, roleId);
}

export async function reorderGuildRoles(
  session: AuthSession,
  guildId: GuildId,
  roleIds: WorkspaceRoleId[],
): Promise<ModerationResult> {
  return workspaceApi.reorderGuildRoles(session, guildId, roleIds);
}

export async function assignGuildRole(
  session: AuthSession,
  guildId: GuildId,
  roleId: WorkspaceRoleId,
  userId: UserId,
): Promise<ModerationResult> {
  return workspaceApi.assignGuildRole(session, guildId, roleId, userId);
}

export async function unassignGuildRole(
  session: AuthSession,
  guildId: GuildId,
  roleId: WorkspaceRoleId,
  userId: UserId,
): Promise<ModerationResult> {
  return workspaceApi.unassignGuildRole(session, guildId, roleId, userId);
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
  return voiceApi.issueVoiceToken(session, guildId, channelId, input);
}
