import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import { userIdFromInput } from "../src/domain/chat";
import { createFriendsApi } from "../src/lib/api-friends";

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

describe("api-friends", () => {
  const session = {
    accessToken: accessTokenFromInput("A".repeat(64)),
    refreshToken: refreshTokenFromInput("B".repeat(64)),
    expiresAtUnix: 2_000_000_000,
  };

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetchFriends sends bounded request and maps strict friend list DTO", async () => {
    const requestJson = vi.fn(async () => ({
      friends: [
        {
          user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          username: "friend_one",
          created_at_unix: 1_700_000_000,
        },
      ],
    }));

    const api = createFriendsApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchFriends(session)).resolves.toEqual([
      {
        userId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        username: "friend_one",
        createdAtUnix: 1_700_000_000,
      },
    ]);
    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: "/friends",
      accessToken: session.accessToken,
    });
  });

  it("createFriendRequest sends recipient id and parses create DTO", async () => {
    const requestJson = vi.fn(async () => ({
      request_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
      recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
      created_at_unix: 1_700_000_100,
    }));

    const api = createFriendsApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const recipientUserId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB2");
    await expect(api.createFriendRequest(session, recipientUserId)).resolves.toMatchObject({
      requestId: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      recipientUserId: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: "/friends/requests",
      accessToken: session.accessToken,
      body: { recipient_user_id: recipientUserId },
    });
  });

  it("acceptFriendRequest fails closed on invalid response shape", async () => {
    const api = createFriendsApi({
      requestJson: vi.fn(async () => ({ accepted: "yes" })),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.acceptFriendRequest(session, "01ARZ3NDEKTSV4RRFFQ69G5FB0")).rejects.toMatchObject({
      status: 500,
      code: "invalid_friend_accept_shape",
    });
  });

  it("delete/remove friend delegate to no-content primitive", async () => {
    const requestNoContent = vi.fn(async () => undefined);
    const api = createFriendsApi({
      requestJson: vi.fn(async () => null),
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const friendUserId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB3");
    await api.deleteFriendRequest(session, "01ARZ3NDEKTSV4RRFFQ69G5FB0");
    await api.removeFriend(session, friendUserId);

    expect(requestNoContent).toHaveBeenNthCalledWith(1, {
      method: "DELETE",
      path: "/friends/requests/01ARZ3NDEKTSV4RRFFQ69G5FB0",
      accessToken: session.accessToken,
    });
    expect(requestNoContent).toHaveBeenNthCalledWith(2, {
      method: "DELETE",
      path: "/friends/01ARZ3NDEKTSV4RRFFQ69G5FB3",
      accessToken: session.accessToken,
    });
  });
});