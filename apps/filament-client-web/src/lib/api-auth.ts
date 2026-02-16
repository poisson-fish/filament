import {
  type AccessToken,
  type AuthSession,
  type CaptchaToken,
  type Password,
  type RefreshToken,
  type Username,
  authSessionFromResponse,
} from "../domain/auth";
import {
  type UserId,
  type UserLookupRecord,
  type ProfileRecord,
  profileFromResponse,
  userLookupListFromResponse,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface BodyRequest {
  method: "POST" | "PATCH" | "DELETE";
  path: string;
  body: BodyInit;
  accessToken?: AccessToken;
  headers?: Record<string, string>;
}

const MAX_PROFILE_AVATAR_BYTES = 2 * 1024 * 1024;

interface AuthApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  requestJsonWithBody: (request: BodyRequest) => Promise<unknown>;
  requestNoContent: (request: JsonRequest) => Promise<void>;
  createApiError: (status: number, code: string, message: string) => Error;
}

export interface AuthApi {
  registerWithPassword(input: {
    username: Username;
    password: Password;
    captchaToken?: CaptchaToken;
  }): Promise<void>;
  loginWithPassword(input: {
    username: Username;
    password: Password;
  }): Promise<AuthSession>;
  refreshAuthSession(refreshToken: RefreshToken): Promise<AuthSession>;
  logoutAuthSession(refreshToken: RefreshToken): Promise<void>;
  fetchMe(session: AuthSession): Promise<ProfileRecord>;
  fetchUserProfile(session: AuthSession, userId: UserId): Promise<ProfileRecord>;
  updateMyProfile(
    session: AuthSession,
    input: { username?: Username; aboutMarkdown?: string },
  ): Promise<ProfileRecord>;
  uploadMyProfileAvatar(session: AuthSession, file: File): Promise<ProfileRecord>;
  lookupUsersByIds(session: AuthSession, userIds: UserId[]): Promise<UserLookupRecord[]>;
}

export function createAuthApi(input: AuthApiDependencies): AuthApi {
  return {
    async registerWithPassword(payload) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/auth/register",
        body: {
          username: payload.username,
          password: payload.password,
          ...(payload.captchaToken ? { captcha_token: payload.captchaToken } : {}),
        },
      });

      if (
        !dto ||
        typeof dto !== "object" ||
        (dto as { accepted?: unknown }).accepted !== true
      ) {
        throw input.createApiError(
          500,
          "invalid_register_shape",
          "Unexpected register response.",
        );
      }
    },

    async loginWithPassword(payload) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/auth/login",
        body: {
          username: payload.username,
          password: payload.password,
        },
      });

      if (!dto || typeof dto !== "object") {
        throw input.createApiError(500, "invalid_login_shape", "Unexpected login response.");
      }

      return authSessionFromResponse(dto as {
        access_token: string;
        refresh_token: string;
        expires_in_secs: number;
      });
    },

    async refreshAuthSession(refreshToken) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/auth/refresh",
        body: { refresh_token: refreshToken },
      });

      if (!dto || typeof dto !== "object") {
        throw input.createApiError(500, "invalid_refresh_shape", "Unexpected refresh response.");
      }

      return authSessionFromResponse(dto as {
        access_token: string;
        refresh_token: string;
        expires_in_secs: number;
      });
    },

    async logoutAuthSession(refreshToken) {
      await input.requestNoContent({
        method: "POST",
        path: "/auth/logout",
        body: { refresh_token: refreshToken },
      });
    },

    async fetchMe(session) {
      const dto = await input.requestJson({
        method: "GET",
        path: "/auth/me",
        accessToken: session.accessToken,
      });

      if (!dto || typeof dto !== "object") {
        throw input.createApiError(500, "invalid_me_shape", "Unexpected profile response.");
      }

      const userId = (dto as { user_id?: unknown }).user_id;
      const username = (dto as { username?: unknown }).username;
      if (typeof userId !== "string" || typeof username !== "string") {
        throw input.createApiError(500, "invalid_me_shape", "Unexpected profile response.");
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
    },

    async fetchUserProfile(session, userId) {
      const dto = await input.requestJson({
        method: "GET",
        path: `/users/${userId}/profile`,
        accessToken: session.accessToken,
      });
      return profileFromResponse(dto);
    },

    async updateMyProfile(session, payload) {
      const body: Record<string, unknown> = {};
      if (payload.username) {
        body.username = payload.username;
      }
      if (typeof payload.aboutMarkdown === "string") {
        body.about_markdown = payload.aboutMarkdown;
      }
      const dto = await input.requestJson({
        method: "PATCH",
        path: "/users/me/profile",
        accessToken: session.accessToken,
        body,
      });
      return profileFromResponse(dto);
    },

    async uploadMyProfileAvatar(session, file) {
      if (file.size < 1 || file.size > MAX_PROFILE_AVATAR_BYTES) {
        throw input.createApiError(
          400,
          "invalid_request",
          "Avatar size must be within server limits.",
        );
      }
      const headers: Record<string, string> = {};
      if (file.type && file.type.length <= 128) {
        headers["content-type"] = file.type;
      }
      const dto = await input.requestJsonWithBody({
        method: "POST",
        path: "/users/me/profile/avatar",
        accessToken: session.accessToken,
        headers,
        body: file,
      });
      return profileFromResponse(dto);
    },

    async lookupUsersByIds(session, userIds) {
      if (userIds.length < 1 || userIds.length > 64) {
        throw input.createApiError(400, "invalid_request", "user_ids must contain 1-64 values.");
      }
      const dto = await input.requestJson({
        method: "POST",
        path: "/users/lookup",
        accessToken: session.accessToken,
        body: { user_ids: userIds },
      });
      return userLookupListFromResponse(dto);
    },
  };
}