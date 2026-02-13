import { describe, expect, it, vi } from "vitest";
import {
  friendListFromResponse,
  friendRequestListFromResponse,
  userIdFromInput,
} from "../src/domain/chat";
import { createFriendshipsPanelProps } from "../src/features/app-shell/runtime/friendships-panel-props";

describe("app shell friendships panel props", () => {
  it("maps friendship values and handlers", async () => {
    const onSubmitFriendRequest = vi.fn();
    const setFriendRecipientUserIdInput = vi.fn();
    const onAcceptIncomingFriendRequest = vi.fn();
    const onDismissFriendRequest = vi.fn();
    const onRemoveFriendship = vi.fn();

    const panelProps = createFriendshipsPanelProps({
      friendRecipientUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
      friendRequests: friendRequestListFromResponse({ incoming: [], outgoing: [] }),
      friends: friendListFromResponse({
        friends: [
          {
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
            username: "filament",
            created_at_unix: 10,
          },
        ],
      }),
      isRunningFriendAction: false,
      friendStatus: "ready",
      friendError: "",
      onSubmitFriendRequest,
      setFriendRecipientUserIdInput,
      onAcceptIncomingFriendRequest,
      onDismissFriendRequest,
      onRemoveFriendship,
    });

    expect(panelProps.friendRecipientUserIdInput).toBe(
      "01ARZ3NDEKTSV4RRFFQ69G5FAA",
    );
    expect(panelProps.friends).toHaveLength(1);
    expect(panelProps.friendStatus).toBe("ready");

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onSubmitFriendRequest(submitEvent);
    expect(onSubmitFriendRequest).toHaveBeenCalledWith(submitEvent);

    panelProps.setFriendRecipientUserIdInput("next-user");
    expect(setFriendRecipientUserIdInput).toHaveBeenCalledWith("next-user");

    await panelProps.onAcceptIncomingFriendRequest("req-1");
    expect(onAcceptIncomingFriendRequest).toHaveBeenCalledWith("req-1");

    await panelProps.onDismissFriendRequest("req-2");
    expect(onDismissFriendRequest).toHaveBeenCalledWith("req-2");

    const friendUserId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");
    await panelProps.onRemoveFriendship(friendUserId);
    expect(onRemoveFriendship).toHaveBeenCalledWith(friendUserId);
  });
});
