import {
  accessTokenFromInput,
  captchaTokenFromInput,
  passwordFromInput,
  refreshTokenFromInput,
  usernameFromInput,
} from "../src/domain/auth";
import { userIdFromInput } from "../src/domain/chat";
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
    const requestJsonWithBody = vi.fn(async () => null);
    const requestNoContent = vi.fn(async () => undefined);
    const api = createAuthApi({
      requestJson,
      requestJsonWithBody,
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
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
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
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
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
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

  it("loginWithPassword includes captcha token when provided", async () => {
    const requestJson = vi.fn(async () => ({
      access_token: "A".repeat(64),
      refresh_token: "B".repeat(64),
      expires_in_secs: 60,
    }));
    const api = createAuthApi({
      requestJson,
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    await api.loginWithPassword({
      username: usernameFromInput("valid_user"),
      password: passwordFromInput("supersecure123"),
      captchaToken: captchaTokenFromInput("C".repeat(24)),
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: "/auth/login",
      body: {
        username: "valid_user",
        password: "supersecure123",
        captcha_token: "C".repeat(24),
      },
    });
  });

  it("logoutAuthSession delegates to no-content request primitive", async () => {
    const requestNoContent = vi.fn(async () => undefined);
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
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
      banner_version: 2,
    }));
    const api = createAuthApi({
      requestJson,
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
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
      bannerVersion: 2,
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
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    await expect(
      api.fetchMe({
        accessToken: accessTokenFromInput("A".repeat(64)),
        refreshToken: refreshTokenFromInput("B".repeat(64)),
        expiresAtUnix: 2_000_000_000,
      }),
    ).rejects.toMatchObject({ status: 500, code: "invalid_me_shape" });
  });

  it("fetchUserProfile requests authenticated user profile by id", async () => {
    const requestJson = vi.fn(async () => ({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "target_user",
      about_markdown: "Profile",
      about_markdown_tokens: [],
      avatar_version: 3,
      banner_version: 1,
    }));
    const api = createAuthApi({
      requestJson,
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await expect(api.fetchUserProfile(session, userId)).resolves.toMatchObject({
      userId,
      username: "target_user",
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/users/${userId}/profile`,
      accessToken: "A".repeat(64),
    });
  });

  it("updateMyProfile maps patch payload fields to server shape", async () => {
    const requestJson = vi.fn(async () => ({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "renamed_user",
      about_markdown: "Updated",
      about_markdown_tokens: [],
      avatar_version: 1,
      banner_version: 0,
    }));
    const api = createAuthApi({
      requestJson,
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    await api.updateMyProfile(
      {
        accessToken: accessTokenFromInput("A".repeat(64)),
        refreshToken: refreshTokenFromInput("B".repeat(64)),
        expiresAtUnix: 2_000_000_000,
      },
      {
        username: usernameFromInput("renamed_user"),
        aboutMarkdown: "Updated",
      },
    );

    expect(requestJson).toHaveBeenCalledWith({
      method: "PATCH",
      path: "/users/me/profile",
      accessToken: "A".repeat(64),
      body: {
        username: "renamed_user",
        about_markdown: "Updated",
      },
    });
  });

  it("uploadMyProfileAvatar enforces strict size bounds", async () => {
    const requestJsonWithBody = vi.fn(async () => null);
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    await expect(
      api.uploadMyProfileAvatar(
        {
          accessToken: accessTokenFromInput("A".repeat(64)),
          refreshToken: refreshTokenFromInput("B".repeat(64)),
          expiresAtUnix: 2_000_000_000,
        },
        new File([new Uint8Array(0)], "avatar.png", { type: "image/png" }),
      ),
    ).rejects.toMatchObject({ status: 400, code: "invalid_request" });

    expect(requestJsonWithBody).not.toHaveBeenCalled();
  });

  it("uploadMyProfileAvatar sends authenticated body request and maps profile DTO", async () => {
    const requestJsonWithBody = vi.fn(async () => ({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "valid_user",
      about_markdown: "Hello",
      about_markdown_tokens: [],
      avatar_version: 5,
      banner_version: 6,
    }));

    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    const file = new File([new Uint8Array([1, 2, 3])], "avatar.png", { type: "image/png" });
    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };

    await expect(api.uploadMyProfileAvatar(session, file)).resolves.toMatchObject({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      avatarVersion: 5,
      bannerVersion: 6,
    });
    expect(requestJsonWithBody).toHaveBeenCalledWith({
      method: "POST",
      path: "/users/me/profile/avatar",
      accessToken: "A".repeat(64),
      headers: {
        "content-type": "image/png",
      },
      body: file,
      timeoutMs: 30_000,
    });
  });

  it("lookupUsersByIds enforces 1-64 ids bounds", async () => {
    const api = createAuthApi({
      requestJson: vi.fn(async () => ({ users: [] })),
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    await expect(
      api.lookupUsersByIds(
        {
          accessToken: accessTokenFromInput("A".repeat(64)),
          refreshToken: refreshTokenFromInput("B".repeat(64)),
          expiresAtUnix: 2_000_000_000,
        },
        [],
      ),
    ).rejects.toMatchObject({ status: 400, code: "invalid_request" });
  });

  it("uploadMyProfileBanner enforces size and MIME bounds", async () => {
    const requestJsonWithBody = vi.fn(async () => null);
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };

    await expect(
      api.uploadMyProfileBanner(
        session,
        new File([new Uint8Array(0)], "banner.png", { type: "image/png" }),
      ),
    ).rejects.toMatchObject({ status: 400, code: "invalid_request" });

    await expect(
      api.uploadMyProfileBanner(
        session,
        new File([new Uint8Array([1, 2, 3])], "banner.svg", { type: "image/svg+xml" }),
      ),
    ).rejects.toMatchObject({ status: 400, code: "invalid_request" });

    expect(requestJsonWithBody).not.toHaveBeenCalled();
  });

  it("uploadMyProfileBanner sends authenticated body request and maps profile DTO", async () => {
    const requestJsonWithBody = vi.fn(async () => ({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "valid_user",
      about_markdown: "Hello",
      about_markdown_tokens: [],
      avatar_version: 5,
      banner_version: 8,
    }));

    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.filament.local",
    });

    const file = new File([new Uint8Array([1, 2, 3])], "banner.png", { type: "image/png" });
    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };

    await expect(api.uploadMyProfileBanner(session, file)).resolves.toMatchObject({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      bannerVersion: 8,
    });
    expect(requestJsonWithBody).toHaveBeenCalledWith({
      method: "POST",
      path: "/users/me/profile/banner",
      accessToken: "A".repeat(64),
      headers: {
        "content-type": "image/png",
      },
      body: file,
      timeoutMs: 30_000,
    });
  });

  it("profileAvatarUrl normalizes version and uses configured base URL", () => {
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.example.test",
    });

    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    expect(api.profileAvatarUrl(userId, -4.8)).toBe(
      "https://api.example.test/users/01ARZ3NDEKTSV4RRFFQ69G5FAV/avatar?v=0",
    );
    expect(api.profileAvatarUrl(userId, 6.9)).toBe(
      "https://api.example.test/users/01ARZ3NDEKTSV4RRFFQ69G5FAV/avatar?v=6",
    );
  });

  it("profileBannerUrl normalizes version and uses configured base URL", () => {
    const api = createAuthApi({
      requestJson: vi.fn(async () => null),
      requestJsonWithBody: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      apiBaseUrl: () => "https://api.example.test",
    });

    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    expect(api.profileBannerUrl(userId, -4.8)).toBe(
      "https://api.example.test/users/01ARZ3NDEKTSV4RRFFQ69G5FAV/banner?v=0",
    );
    expect(api.profileBannerUrl(userId, 6.9)).toBe(
      "https://api.example.test/users/01ARZ3NDEKTSV4RRFFQ69G5FAV/banner?v=6",
    );
  });
});
