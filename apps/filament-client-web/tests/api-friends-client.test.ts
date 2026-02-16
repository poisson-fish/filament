import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  friendListFromResponse,
  friendRequestCreateFromResponse,
  friendRequestListFromResponse,
  userIdFromInput,
} from "../src/domain/chat";
import type { FriendsApi } from "../src/lib/api-friends";
import { createFriendsClient } from "../src/lib/api-friends-client";

describe("api-friends-client", () => {
  function createSession() {
    return {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
  }

  function createFriendsApiStub(overrides?: Partial<FriendsApi>): FriendsApi {
    const api: FriendsApi = {
      fetchFriends: vi.fn(async () =>
        friendListFromResponse({
          friends: [
            {
              user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
              username: "friend_user",
              created_at_unix: 1_700_000_000,
            },
          ],
        }),
      ),
      fetchFriendRequests: vi.fn(async () =>
        friendRequestListFromResponse({
          received: [],
          sent: [],
        }),
      ),
      createFriendRequest: vi.fn(async () =>
        friendRequestCreateFromResponse({
          request_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          created_at_unix: 1_700_000_100,
        }),
      ),
      acceptFriendRequest: vi.fn(async () => undefined),
      deleteFriendRequest: vi.fn(async () => undefined),
      removeFriend: vi.fn(async () => undefined),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates fetchFriends through friends API", async () => {
    const expectedFriends = friendListFromResponse({
      friends: [
        {
          user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          username: "friend_user",
          created_at_unix: 1_700_000_000,
        },
      ],
    });
    const fetchFriends = vi.fn(async () => expectedFriends);
    const client = createFriendsClient({
      friendsApi: createFriendsApiStub({ fetchFriends }),
    });
    const session = createSession();

    await expect(client.fetchFriends(session)).resolves.toBe(expectedFriends);
    expect(fetchFriends).toHaveBeenCalledWith(session);
  });

  it("delegates createFriendRequest and returns upstream value", async () => {
    const expectedCreate = friendRequestCreateFromResponse({
      request_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
      recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      created_at_unix: 1_700_000_100,
    });
    const createFriendRequest = vi.fn(async () => expectedCreate);
    const client = createFriendsClient({
      friendsApi: createFriendsApiStub({ createFriendRequest }),
    });
    const session = createSession();
    const recipientUserId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await expect(client.createFriendRequest(session, recipientUserId)).resolves.toBe(expectedCreate);
    expect(createFriendRequest).toHaveBeenCalledWith(session, recipientUserId);
  });

  it("delegates removeFriend", async () => {
    const removeFriend = vi.fn(async () => undefined);
    const client = createFriendsClient({
      friendsApi: createFriendsApiStub({ removeFriend }),
    });
    const session = createSession();
    const friendUserId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await client.removeFriend(session, friendUserId);

    expect(removeFriend).toHaveBeenCalledWith(session, friendUserId);
  });
});