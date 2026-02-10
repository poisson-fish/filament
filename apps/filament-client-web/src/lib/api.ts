import {
  type AuthSession,
  type Password,
  type Username,
  authSessionFromResponse,
} from "../domain/auth";
import {
  type ChannelId,
  type ChannelName,
  type GuildId,
  type GuildName,
  type MessageContent,
  type MessageHistory,
  type MessageId,
  type MessageRecord,
  type ReactionEmoji,
  type ReactionRecord,
  type SearchQuery,
  type SearchResults,
  channelFromResponse,
  guildFromResponse,
  messageFromResponse,
  messageHistoryFromResponse,
  reactionFromResponse,
  searchResultsFromResponse,
} from "../domain/chat";
import { bearerHeader } from "./session";

const DEFAULT_API_ORIGIN = "https://api.filament.local";
const MAX_RESPONSE_BYTES = 64 * 1024;
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
  accessToken?: AuthSession["accessToken"];
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
  const config = apiConfig();
  const headers: Record<string, string> = {};

  if (request.body !== undefined) {
    headers["content-type"] = "application/json";
  }
  if (request.accessToken) {
    headers.authorization = bearerHeader(request.accessToken);
  }

  let response: Response;
  try {
    response = await fetchWithTimeout(
      `${config.baseUrl}${request.path}`,
      {
        method: request.method,
        headers,
        body: request.body === undefined ? undefined : JSON.stringify(request.body),
        credentials: "omit",
      },
      REQUEST_TIMEOUT_MS,
    );
  } catch {
    throw new ApiError(0, "network_error", "Unable to reach Filament API.");
  }

  const data = await readBoundedJson(response);
  if (!response.ok) {
    const code = readApiError(data);
    throw new ApiError(response.status, code, code);
  }
  return data;
}

export async function registerWithPassword(input: {
  username: Username;
  password: Password;
}): Promise<void> {
  const dto = await requestJson({
    method: "POST",
    path: "/auth/register",
    body: {
      username: input.username,
      password: input.password,
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

export async function fetchMe(session: AuthSession): Promise<{ userId: string; username: string }> {
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
    userId,
    username,
  };
}

export async function createGuild(session: AuthSession, input: { name: GuildName }): Promise<{
  guildId: GuildId;
  name: GuildName;
}> {
  const dto = await requestJson({
    method: "POST",
    path: "/guilds",
    accessToken: session.accessToken,
    body: { name: input.name },
  });
  return guildFromResponse(dto);
}

export async function createChannel(
  session: AuthSession,
  guildId: GuildId,
  input: { name: ChannelName },
): Promise<{ channelId: ChannelId; name: ChannelName }> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels`,
    accessToken: session.accessToken,
    body: { name: input.name },
  });
  return channelFromResponse(dto);
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
  input: { content: MessageContent },
): Promise<MessageRecord> {
  const dto = await requestJson({
    method: "POST",
    path: `/guilds/${guildId}/channels/${channelId}/messages`,
    accessToken: session.accessToken,
    body: { content: input.content },
  });
  return messageFromResponse(dto);
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
