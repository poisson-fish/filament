import {
  decodeFriendGatewayEvent,
} from "../src/lib/gateway-friend-events";

const DEFAULT_REQUEST_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_FRIEND_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("decodeFriendGatewayEvent", () => {
  it("decodes valid friend_request_create payload", () => {
    const result = decodeFriendGatewayEvent("friend_request_create", {
      request_id: DEFAULT_REQUEST_ID,
      sender_user_id: DEFAULT_USER_ID,
      sender_username: "sender",
      recipient_user_id: DEFAULT_FRIEND_USER_ID,
      recipient_username: "recipient",
      created_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "friend_request_create",
      payload: {
        requestId: DEFAULT_REQUEST_ID,
        senderUserId: DEFAULT_USER_ID,
        senderUsername: "sender",
        recipientUserId: DEFAULT_FRIEND_USER_ID,
        recipientUsername: "recipient",
        createdAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid friend_request_update payload", () => {
    const result = decodeFriendGatewayEvent("friend_request_update", {
      request_id: DEFAULT_REQUEST_ID,
      state: "pending",
      user_id: DEFAULT_USER_ID,
      friend_user_id: DEFAULT_FRIEND_USER_ID,
      friend_username: "friend",
      friendship_created_at_unix: 1710000001,
      updated_at_unix: 1710000002,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid friend_remove payload", () => {
    const result = decodeFriendGatewayEvent("friend_remove", {
      user_id: DEFAULT_USER_ID,
      friend_user_id: "bad-ulid",
      removed_at_unix: 1710000003,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeFriendGatewayEvent("friend_unknown", {
      user_id: DEFAULT_USER_ID,
      friend_user_id: DEFAULT_FRIEND_USER_ID,
      removed_at_unix: 1710000003,
    });

    expect(result).toBeNull();
  });

  it("fails closed for hostile prototype event type", () => {
    const result = decodeFriendGatewayEvent("__proto__", {
      user_id: DEFAULT_USER_ID,
      friend_user_id: DEFAULT_FRIEND_USER_ID,
      removed_at_unix: 1710000003,
    });

    expect(result).toBeNull();
  });
});