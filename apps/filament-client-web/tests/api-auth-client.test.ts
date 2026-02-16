import {
  accessTokenFromInput,
  refreshTokenFromInput,
  usernameFromInput,
  passwordFromInput,
} from "../src/domain/auth";
import {
  profileFromResponse,
  userIdFromInput,
  userLookupListFromResponse,
} from "../src/domain/chat";
import type { AuthApi } from "../src/lib/api-auth";
import { createAuthClient } from "../src/lib/api-auth-client";

describe("api-auth-client", () => {
  function createSession() {
    return {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
  }

  function createAuthApiStub(overrides?: Partial<AuthApi>): AuthApi {
    const defaultProfile = profileFromResponse({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "valid_user",
      about_markdown: "",
      about_markdown_tokens: [],
      avatar_version: 0,
    });

    const api: AuthApi = {
      registerWithPassword: vi.fn(async () => undefined),
      loginWithPassword: vi.fn(async () => createSession()),
      refreshAuthSession: vi.fn(async () => createSession()),
      logoutAuthSession: vi.fn(async () => undefined),
      fetchMe: vi.fn(async () => defaultProfile),
      fetchUserProfile: vi.fn(async () => defaultProfile),
      updateMyProfile: vi.fn(async () => defaultProfile),
      uploadMyProfileAvatar: vi.fn(async () => defaultProfile),
      lookupUsersByIds: vi.fn(async () =>
        userLookupListFromResponse({
          users: [
            {
              user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
              username: "valid_user",
              avatar_version: 0,
            },
          ],
        }),
      ),
      profileAvatarUrl: vi.fn((userId, avatarVersion) =>
        `https://api.filament.local/users/${userId}/avatar?v=${avatarVersion}`,
      ),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates registerWithPassword through auth API", async () => {
    const registerWithPassword = vi.fn(async () => undefined);
    const authClient = createAuthClient({
      authApi: createAuthApiStub({ registerWithPassword }),
    });

    await authClient.registerWithPassword({
      username: usernameFromInput("valid_user"),
      password: passwordFromInput("supersecure123"),
    });

    expect(registerWithPassword).toHaveBeenCalledWith({
      username: "valid_user",
      password: "supersecure123",
    });
  });

  it("delegates lookupUsersByIds and returns upstream value", async () => {
    const expectedLookup = userLookupListFromResponse({
      users: [
        {
          user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          username: "valid_user",
          avatar_version: 2,
        },
      ],
    });
    const lookupUsersByIds = vi.fn(async () => expectedLookup);
    const authClient = createAuthClient({
      authApi: createAuthApiStub({ lookupUsersByIds }),
    });
    const session = createSession();
    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await expect(authClient.lookupUsersByIds(session, [userId])).resolves.toBe(expectedLookup);
    expect(lookupUsersByIds).toHaveBeenCalledWith(session, [userId]);
  });

  it("delegates profileAvatarUrl construction", () => {
    const profileAvatarUrl = vi.fn((userId: string, avatarVersion: number) =>
      `https://cdn.filament.local/${userId}?v=${avatarVersion}`,
    );
    const authClient = createAuthClient({
      authApi: createAuthApiStub({ profileAvatarUrl }),
    });
    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    const result = authClient.profileAvatarUrl(userId, 7);

    expect(result).toBe("https://cdn.filament.local/01ARZ3NDEKTSV4RRFFQ69G5FAV?v=7");
    expect(profileAvatarUrl).toHaveBeenCalledWith(userId, 7);
  });
});