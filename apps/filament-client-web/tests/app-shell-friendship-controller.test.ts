import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  friendRequestCreateFromResponse,
  friendListFromResponse,
  friendRequestListFromResponse,
  type FriendRecord,
  userIdFromInput,
} from "../src/domain/chat";
import { createFriendshipController } from "../src/features/app-shell/controllers/friendship-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

function deferred<T>() {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((reason?: unknown) => void) | undefined;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
    reject: (reason?: unknown) => reject?.(reason),
  };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell friendship controller", () => {
  it("submits friend requests and refreshes friend state", async () => {
    const [session] = createSignal(SESSION);
    const [friendRecipientUserIdInput, setFriendRecipientUserIdInput] = createSignal(
      " 01ARZ3NDEKTSV4RRFFQ69G5FAA ",
    );
    const [isRunningFriendAction, setRunningFriendAction] = createSignal(false);
    const [friends, setFriends] = createSignal<FriendRecord[]>([]);
    const [friendRequests, setFriendRequests] = createSignal(
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    const [friendStatus, setFriendStatus] = createSignal("");
    const [friendError, setFriendError] = createSignal("");

    const fetchFriendsMock = vi.fn(async () =>
      friendListFromResponse({
        friends: [
          {
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
            username: "bob",
            created_at_unix: 10,
          },
        ],
      }),
    );
    const fetchFriendRequestsMock = vi.fn(async () =>
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    const createFriendRequestMock = vi.fn(async () =>
      friendRequestCreateFromResponse({
        request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAC",
        recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        created_at_unix: 5,
      }),
    );

    const controller = createRoot(() =>
      createFriendshipController(
        {
          session,
          friendRecipientUserIdInput,
          isRunningFriendAction,
          setFriends,
          setFriendRequests,
          setRunningFriendAction,
          setFriendStatus,
          setFriendError,
          setFriendRecipientUserIdInput,
        },
        {
          fetchFriends: fetchFriendsMock,
          fetchFriendRequests: fetchFriendRequestsMock,
          createFriendRequest: createFriendRequestMock,
        },
      ),
    );

    await flush();
    expect(fetchFriendsMock).toHaveBeenCalledTimes(1);
    expect(friends()).toHaveLength(1);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;
    await controller.submitFriendRequest(submitEvent);

    expect(submitEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(createFriendRequestMock).toHaveBeenCalledWith(
      SESSION,
      userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA"),
    );
    expect(friendRecipientUserIdInput()).toBe("");
    expect(friendStatus()).toBe("Friend request sent.");
    expect(friendError()).toBe("");
    expect(isRunningFriendAction()).toBe(false);
  });

  it("cancels stale friend-directory refreshes after auth reset", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [friendRecipientUserIdInput, setFriendRecipientUserIdInput] = createSignal("");
    const [isRunningFriendAction, setRunningFriendAction] = createSignal(false);
    const [friends, setFriends] = createSignal(
      friendListFromResponse({
        friends: [
          {
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAD",
            username: "stale",
            created_at_unix: 1,
          },
        ],
      }),
    );
    const [friendRequests, setFriendRequests] = createSignal(
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    const [friendStatus, setFriendStatus] = createSignal("stale");
    const [friendError, setFriendError] = createSignal("stale");

    const pendingFriends = deferred<ReturnType<typeof friendListFromResponse>>();
    const fetchFriendsMock = vi.fn(() => pendingFriends.promise);
    const fetchFriendRequestsMock = vi.fn(async () =>
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );

    const dispose = createRoot((rootDispose) => {
      createFriendshipController(
        {
          session,
          friendRecipientUserIdInput,
          isRunningFriendAction,
          setFriends,
          setFriendRequests,
          setRunningFriendAction,
          setFriendStatus,
          setFriendError,
          setFriendRecipientUserIdInput,
        },
        {
          fetchFriends: fetchFriendsMock,
          fetchFriendRequests: fetchFriendRequestsMock,
        },
      );
      return rootDispose;
    });

    setSession(null);
    pendingFriends.resolve(
      friendListFromResponse({
        friends: [
          {
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAE",
            username: "new",
            created_at_unix: 2,
          },
        ],
      }),
    );
    await flush();

    expect(friends()).toEqual([]);
    expect(friendRequests()).toEqual(
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    expect(friendStatus()).toBe("");
    expect(friendError()).toBe("");
    expect(isRunningFriendAction()).toBe(false);

    dispose();
  });
});
