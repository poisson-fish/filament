import {
  dispatchFriendGatewayEvent,
} from "../src/lib/gateway-friend-dispatch";

const DEFAULT_REQUEST_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_FRIEND_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("dispatchFriendGatewayEvent", () => {
  it("dispatches decoded friend events to matching handlers", () => {
    const onFriendRequestCreate = vi.fn();

    const handled = dispatchFriendGatewayEvent(
      "friend_request_create",
      {
        request_id: DEFAULT_REQUEST_ID,
        sender_user_id: DEFAULT_USER_ID,
        sender_username: "sender",
        recipient_user_id: DEFAULT_FRIEND_USER_ID,
        recipient_username: "recipient",
        created_at_unix: 1710000001,
      },
      { onFriendRequestCreate },
    );

    expect(handled).toBe(true);
    expect(onFriendRequestCreate).toHaveBeenCalledTimes(1);
    expect(onFriendRequestCreate).toHaveBeenCalledWith({
      requestId: DEFAULT_REQUEST_ID,
      senderUserId: DEFAULT_USER_ID,
      senderUsername: "sender",
      recipientUserId: DEFAULT_FRIEND_USER_ID,
      recipientUsername: "recipient",
      createdAtUnix: 1710000001,
    });
  });

  it("fails closed for known friend types with invalid payloads", () => {
    const onFriendRequestUpdate = vi.fn();

    const handled = dispatchFriendGatewayEvent(
      "friend_request_update",
      {
        request_id: DEFAULT_REQUEST_ID,
        state: "pending",
        user_id: DEFAULT_USER_ID,
        friend_user_id: DEFAULT_FRIEND_USER_ID,
        friend_username: "friend",
        friendship_created_at_unix: 1710000001,
        updated_at_unix: 1710000002,
      },
      { onFriendRequestUpdate },
    );

    expect(handled).toBe(true);
    expect(onFriendRequestUpdate).not.toHaveBeenCalled();
  });

  it("returns false for non-friend event types", () => {
    const onFriendRemove = vi.fn();

    const handled = dispatchFriendGatewayEvent(
      "message_create",
      {},
      { onFriendRemove },
    );

    expect(handled).toBe(false);
    expect(onFriendRemove).not.toHaveBeenCalled();
  });
});
