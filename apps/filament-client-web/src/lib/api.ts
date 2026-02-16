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
  type MediaPublishSource,
  type PublicGuildDirectory,
  type ModerationResult,
  type PermissionName,
  type RoleName,
  type ProfileRecord,
  type WorkspaceRoleId,
  type WorkspaceRoleName,
  type UserLookupRecord,
  type UserId,
  type VoiceTokenRecord,
} from "../domain/chat";
import { bearerHeader } from "./session";
import { createAuthApi } from "./api-auth";
import { createAuthClient } from "./api-auth-client";
import { createFriendsApi } from "./api-friends";
import { createFriendsClient } from "./api-friends-client";
import { createMessagesClient } from "./api-messages-client";
import { createMessagesApi } from "./api-messages";
import { createSystemClient } from "./api-system-client";
import { createSystemApi } from "./api-system";
import { createVoiceClient } from "./api-voice-client";
import { createVoiceApi } from "./api-voice";
import { createWorkspaceClient } from "./api-workspace-client";
import { createWorkspaceApi } from "./api-workspace";

const DEFAULT_API_ORIGIN = "https://api.filament.local";
const MAX_RESPONSE_BYTES = 64 * 1024;
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
  requestJsonWithBody,
  requestNoContent,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
  apiBaseUrl() {
    return apiConfig().baseUrl;
  },
});

const authClient = createAuthClient({ authApi });

const friendsApi = createFriendsApi({
  requestJson,
  requestNoContent,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

const friendsClient = createFriendsClient({
  friendsApi,
});

const messagesApi = createMessagesApi({
  requestJson,
  requestNoContent,
  requestJsonWithBody,
  requestBinary,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
  isApiErrorCode(error, code) {
    return error instanceof ApiError && error.code === code;
  },
});

const messagesClient = createMessagesClient({
  messagesApi,
});

const voiceApi = createVoiceApi({
  requestJson,
});

const voiceClient = createVoiceClient({
  voiceApi,
});

const workspaceApi = createWorkspaceApi({
  requestJson,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

const workspaceClient = createWorkspaceClient({
  workspaceApi,
});

const systemApi = createSystemApi({
  requestJson,
  createApiError(status, code, message) {
    return new ApiError(status, code, message);
  },
});

const systemClient = createSystemClient({
  systemApi,
});

export const registerWithPassword = authClient.registerWithPassword;
export const loginWithPassword = authClient.loginWithPassword;
export const refreshAuthSession = authClient.refreshAuthSession;
export const logoutAuthSession = authClient.logoutAuthSession;
export const fetchMe = authClient.fetchMe;
export const fetchUserProfile = authClient.fetchUserProfile;
export const updateMyProfile = authClient.updateMyProfile;
export const uploadMyProfileAvatar = authClient.uploadMyProfileAvatar;
export const profileAvatarUrl = authClient.profileAvatarUrl;
export const lookupUsersByIds = authClient.lookupUsersByIds;

export const fetchFriends = friendsClient.fetchFriends;
export const fetchFriendRequests = friendsClient.fetchFriendRequests;
export const createFriendRequest = friendsClient.createFriendRequest;
export const acceptFriendRequest = friendsClient.acceptFriendRequest;
export const deleteFriendRequest = friendsClient.deleteFriendRequest;
export const removeFriend = friendsClient.removeFriend;

export const fetchHealth = systemClient.fetchHealth;
export const echoMessage = systemClient.echoMessage;

export const createGuild = workspaceClient.createGuild;
export const fetchGuilds = workspaceClient.fetchGuilds;
export const updateGuild = workspaceClient.updateGuild;
export const fetchPublicGuildDirectory = workspaceClient.fetchPublicGuildDirectory;
export const joinPublicGuild = workspaceClient.joinPublicGuild;
export const fetchGuildChannels = workspaceClient.fetchGuildChannels;
export const createChannel = workspaceClient.createChannel;
export const fetchChannelPermissionSnapshot = workspaceClient.fetchChannelPermissionSnapshot;

export const fetchChannelMessages = messagesClient.fetchChannelMessages;
export const createChannelMessage = messagesClient.createChannelMessage;
export const editChannelMessage = messagesClient.editChannelMessage;
export const deleteChannelMessage = messagesClient.deleteChannelMessage;
export const searchGuildMessages = messagesClient.searchGuildMessages;
export const rebuildGuildSearchIndex = messagesClient.rebuildGuildSearchIndex;
export const reconcileGuildSearchIndex = messagesClient.reconcileGuildSearchIndex;
export const addMessageReaction = messagesClient.addMessageReaction;
export const removeMessageReaction = messagesClient.removeMessageReaction;
export const uploadChannelAttachment = messagesClient.uploadChannelAttachment;
export const downloadChannelAttachment = messagesClient.downloadChannelAttachment;
export const downloadChannelAttachmentPreview = messagesClient.downloadChannelAttachmentPreview;
export const deleteChannelAttachment = messagesClient.deleteChannelAttachment;

export const addGuildMember = workspaceClient.addGuildMember;
export const updateGuildMemberRole = workspaceClient.updateGuildMemberRole;
export const kickGuildMember = workspaceClient.kickGuildMember;
export const banGuildMember = workspaceClient.banGuildMember;
export const setChannelRoleOverride = workspaceClient.setChannelRoleOverride;
export const fetchGuildRoles = workspaceClient.fetchGuildRoles;
export const createGuildRole = workspaceClient.createGuildRole;
export const updateGuildRole = workspaceClient.updateGuildRole;
export const deleteGuildRole = workspaceClient.deleteGuildRole;
export const reorderGuildRoles = workspaceClient.reorderGuildRoles;
export const assignGuildRole = workspaceClient.assignGuildRole;
export const unassignGuildRole = workspaceClient.unassignGuildRole;

export const issueVoiceToken = voiceClient.issueVoiceToken;
