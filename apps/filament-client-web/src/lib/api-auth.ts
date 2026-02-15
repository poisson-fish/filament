import {
  type AuthSession,
  type CaptchaToken,
  type Password,
  type RefreshToken,
  type Username,
  authSessionFromResponse,
} from "../domain/auth";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
}

interface AuthApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
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
  };
}