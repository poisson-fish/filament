import { type AuthSession } from "../domain/auth";
import {
  type FriendRecord,
  type FriendRequestCreateResult,
  type FriendRequestList,
  type UserId,
} from "../domain/chat";
import type { FriendsApi } from "./api-friends";

interface FriendsClientDependencies {
  friendsApi: FriendsApi;
}

export interface FriendsClient {
  fetchFriends(session: AuthSession): Promise<FriendRecord[]>;
  fetchFriendRequests(session: AuthSession): Promise<FriendRequestList>;
  createFriendRequest(
    session: AuthSession,
    recipientUserId: UserId,
  ): Promise<FriendRequestCreateResult>;
  acceptFriendRequest(session: AuthSession, requestId: string): Promise<void>;
  deleteFriendRequest(session: AuthSession, requestId: string): Promise<void>;
  removeFriend(session: AuthSession, friendUserId: UserId): Promise<void>;
}

export function createFriendsClient(input: FriendsClientDependencies): FriendsClient {
  return {
    fetchFriends(session) {
      return input.friendsApi.fetchFriends(session);
    },

    fetchFriendRequests(session) {
      return input.friendsApi.fetchFriendRequests(session);
    },

    createFriendRequest(session, recipientUserId) {
      return input.friendsApi.createFriendRequest(session, recipientUserId);
    },

    acceptFriendRequest(session, requestId) {
      return input.friendsApi.acceptFriendRequest(session, requestId);
    },

    deleteFriendRequest(session, requestId) {
      return input.friendsApi.deleteFriendRequest(session, requestId);
    },

    removeFriend(session, friendUserId) {
      return input.friendsApi.removeFriend(session, friendUserId);
    },
  };
}