import {
  type AccessToken,
  type AuthSession,
  type Password,
  type Username,
  authSessionFromResponse,
} from "../domain/auth";
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

async function postJson(path: string, body: unknown): Promise<unknown> {
  const config = apiConfig();
  let response: Response;
  try {
    response = await fetchWithTimeout(
      `${config.baseUrl}${path}`,
      {
        method: "POST",
        headers: {
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
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

async function getJson(path: string, accessToken: AccessToken): Promise<unknown> {
  const config = apiConfig();
  let response: Response;
  try {
    response = await fetchWithTimeout(
      `${config.baseUrl}${path}`,
      {
        method: "GET",
        headers: {
          authorization: bearerHeader(accessToken),
        },
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
  const dto = await postJson("/auth/register", {
    username: input.username,
    password: input.password,
  });

  if (!dto || typeof dto !== "object" || (dto as { accepted?: unknown }).accepted !== true) {
    throw new ApiError(500, "invalid_register_shape", "Unexpected register response.");
  }
}

export async function loginWithPassword(input: {
  username: Username;
  password: Password;
}): Promise<AuthSession> {
  const dto = await postJson("/auth/login", {
    username: input.username,
    password: input.password,
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
  const dto = await getJson("/auth/me", session.accessToken);
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
