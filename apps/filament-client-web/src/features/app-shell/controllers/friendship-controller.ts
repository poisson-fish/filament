import { createEffect, untrack, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import { userIdFromInput, type FriendRecord, type FriendRequestList, type UserId } from "../../../domain/chat";
import {
  acceptFriendRequest,
  createFriendRequest,
  deleteFriendRequest,
  fetchFriendRequests,
  fetchFriends,
  removeFriend,
} from "../../../lib/api";
import { mapError } from "../helpers";

export interface FriendshipControllerOptions {
  session: Accessor<AuthSession | null>;
  friendRecipientUserIdInput: Accessor<string>;
  isRunningFriendAction: Accessor<boolean>;
  setFriends: Setter<FriendRecord[]>;
  setFriendRequests: Setter<FriendRequestList>;
  setRunningFriendAction: Setter<boolean>;
  setFriendStatus: Setter<string>;
  setFriendError: Setter<string>;
  setFriendRecipientUserIdInput: Setter<string>;
}

export interface FriendshipControllerDependencies {
  fetchFriends: typeof fetchFriends;
  fetchFriendRequests: typeof fetchFriendRequests;
  createFriendRequest: typeof createFriendRequest;
  acceptFriendRequest: typeof acceptFriendRequest;
  deleteFriendRequest: typeof deleteFriendRequest;
  removeFriend: typeof removeFriend;
}

export interface FriendshipController {
  refreshFriendDirectory: () => Promise<void>;
  submitFriendRequest: (event: SubmitEvent) => Promise<void>;
  acceptIncomingFriendRequest: (requestId: string) => Promise<void>;
  dismissFriendRequest: (requestId: string) => Promise<void>;
  removeFriendship: (friendUserId: UserId) => Promise<void>;
}

const EMPTY_FRIEND_REQUEST_LIST: FriendRequestList = {
  incoming: [],
  outgoing: [],
};

const DEFAULT_FRIENDSHIP_CONTROLLER_DEPENDENCIES: FriendshipControllerDependencies = {
  fetchFriends,
  fetchFriendRequests,
  createFriendRequest,
  acceptFriendRequest,
  deleteFriendRequest,
  removeFriend,
};

export function createFriendshipController(
  options: FriendshipControllerOptions,
  dependencies: Partial<FriendshipControllerDependencies> = {},
): FriendshipController {
  const deps = {
    ...DEFAULT_FRIENDSHIP_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  let friendDirectoryRequestVersion = 0;

  const refreshFriendDirectory = async (): Promise<void> => {
    const session = options.session();
    if (!session) {
      options.setFriends([]);
      options.setFriendRequests(EMPTY_FRIEND_REQUEST_LIST);
      return;
    }

    const requestVersion = ++friendDirectoryRequestVersion;
    options.setFriendError("");
    try {
      const [friendList, requestList] = await Promise.all([
        deps.fetchFriends(session),
        deps.fetchFriendRequests(session),
      ]);
      if (requestVersion !== friendDirectoryRequestVersion) {
        return;
      }
      options.setFriends(friendList);
      options.setFriendRequests(requestList);
    } catch (error) {
      if (requestVersion !== friendDirectoryRequestVersion) {
        return;
      }
      options.setFriendError(mapError(error, "Unable to load friendship state."));
    }
  };

  const submitFriendRequest = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    const session = options.session();
    if (!session || options.isRunningFriendAction()) {
      return;
    }
    options.setRunningFriendAction(true);
    options.setFriendError("");
    options.setFriendStatus("");
    try {
      const recipientUserId = userIdFromInput(options.friendRecipientUserIdInput().trim());
      await deps.createFriendRequest(session, recipientUserId);
      options.setFriendRecipientUserIdInput("");
      await refreshFriendDirectory();
      options.setFriendStatus("Friend request sent.");
    } catch (error) {
      options.setFriendError(mapError(error, "Unable to create friend request."));
    } finally {
      options.setRunningFriendAction(false);
    }
  };

  const acceptIncomingFriendRequest = async (requestId: string): Promise<void> => {
    const session = options.session();
    if (!session || options.isRunningFriendAction()) {
      return;
    }
    options.setRunningFriendAction(true);
    options.setFriendError("");
    options.setFriendStatus("");
    try {
      await deps.acceptFriendRequest(session, requestId);
      await refreshFriendDirectory();
      options.setFriendStatus("Friend request accepted.");
    } catch (error) {
      options.setFriendError(mapError(error, "Unable to accept friend request."));
    } finally {
      options.setRunningFriendAction(false);
    }
  };

  const dismissFriendRequest = async (requestId: string): Promise<void> => {
    const session = options.session();
    if (!session || options.isRunningFriendAction()) {
      return;
    }
    options.setRunningFriendAction(true);
    options.setFriendError("");
    options.setFriendStatus("");
    try {
      await deps.deleteFriendRequest(session, requestId);
      await refreshFriendDirectory();
      options.setFriendStatus("Friend request removed.");
    } catch (error) {
      options.setFriendError(mapError(error, "Unable to remove friend request."));
    } finally {
      options.setRunningFriendAction(false);
    }
  };

  const removeFriendship = async (friendUserId: UserId): Promise<void> => {
    const session = options.session();
    if (!session || options.isRunningFriendAction()) {
      return;
    }
    options.setRunningFriendAction(true);
    options.setFriendError("");
    options.setFriendStatus("");
    try {
      await deps.removeFriend(session, friendUserId);
      await refreshFriendDirectory();
      options.setFriendStatus("Friend removed.");
    } catch (error) {
      options.setFriendError(mapError(error, "Unable to remove friend."));
    } finally {
      options.setRunningFriendAction(false);
    }
  };

  createEffect(() => {
    const session = options.session();
    friendDirectoryRequestVersion += 1;
    if (!session) {
      options.setFriends([]);
      options.setFriendRequests(EMPTY_FRIEND_REQUEST_LIST);
      options.setRunningFriendAction(false);
      options.setFriendStatus("");
      options.setFriendError("");
      return;
    }
    void untrack(() => refreshFriendDirectory());
  });

  return {
    refreshFriendDirectory,
    submitFriendRequest,
    acceptIncomingFriendRequest,
    dismissFriendRequest,
    removeFriendship,
  };
}
