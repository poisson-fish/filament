import {
  accessTokenFromInput,
  captchaTokenFromInput,
  passwordFromInput,
  refreshTokenFromInput,
  usernameFromInput,
} from "../src/domain/auth";
import { createAuthApi } from "../src/lib/api-auth";

class MockApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "MockApiError";
    this.status = status;
    this.code = code;
  }
}

describe("api-auth", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("registerWithPassword sends auth register request and accepts strict shape", async () => {
    const requestJson = vi.fn(async () => ({ accepted: true }));
    const requestNoContent = vi.fn(async () => undefined);
    const api = createAuthApi({
      requestJson,
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await api.registerWithPassword({
      username: usernameFromInput("valid_user"),
      password: passwordFromInput("supersecure123"),
      captchaToken: captchaTokenFromInput("C".repeat(24)),
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: "/auth/register",
      body: {
        username: "valid_user",
        password: "supersecure123",
        captcha_token: "C".repeat(24),
      },
    });
  });

  it("registerWithPassword fails closed on invalid response shape", async () => {
    const api = createAuthApi({
      requestJson: vi.fn(async () => ({ accepted: "yes" })),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.registerWithPassword({
        username: usernameFromInput("valid_user"),
        password: passwordFromInput("supersecure123"),
      }),
    ).rejects.toMatchObject({ status: 500, code: "invalid_register_shape" });
  });

  it("loginWithPassword maps session DTO through auth domain parser", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-02-14T00:00:00.000Z"));
    const expectedExpiresAtUnix = Math.floor(Date.now() / 1000) + 60;

    const api = createAuthApi({
      requestJson: vi.fn(async () => ({
        access_token: "A".repeat(64),
        refresh_token: "B".repeat(64),
        expires_in_secs: 60,
      })),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.loginWithPassword({
        username: usernameFromInput("valid_user"),
        password: passwordFromInput("supersecure123"),
      }),
    ).resolves.toMatchObject({
      accessToken: "A".repeat(64),
      refreshToken: "B".repeat(64),
      expiresAtUnix: expectedExpiresAtUnix,
    });
  });

  it("logoutAuthSession delegates to no-content request primitive", async () => {
    const requestNoContent = vi.fn(async () => undefined);
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await api.logoutAuthSession(refreshTokenFromInput("R".repeat(64)));

    expect(requestNoContent).toHaveBeenCalledWith({
      method: "POST",
      path: "/auth/logout",
      body: { refresh_token: "R".repeat(64) },
    });
  });

  it("fetchMe requests authenticated profile and maps strict DTO fields", async () => {
    const requestJson = vi.fn(async () => ({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "valid_user",
      about_markdown: "Hello",
      about_markdown_tokens: [],
      avatar_version: 4,
    }));
    const api = createAuthApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };

    await expect(api.fetchMe(session)).resolves.toMatchObject({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "valid_user",
      aboutMarkdown: "Hello",
      avatarVersion: 4,
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: "/auth/me",
      accessToken: "A".repeat(64),
    });
  });

  it("fetchMe fails closed on invalid profile shape", async () => {
    const api = createAuthApi({
      requestJson: vi.fn(async () => ({ user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV" })),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.fetchMe({
        accessToken: accessTokenFromInput("A".repeat(64)),
        refreshToken: refreshTokenFromInput("B".repeat(64)),
        expiresAtUnix: 2_000_000_000,
      }),
    ).rejects.toMatchObject({ status: 500, code: "invalid_me_shape" });
  });
});
