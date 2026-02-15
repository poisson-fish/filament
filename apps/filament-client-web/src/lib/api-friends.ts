import {
  type AuthSession,
  type AccessToken,
} from "../domain/auth";
import {
  type FriendRecord,
  type FriendRequestCreateResult,
  type FriendRequestList,
  type UserId,
  friendListFromResponse,
  friendRequestCreateFromResponse,
  friendRequestListFromResponse,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface FriendsApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  requestNoContent: (request: JsonRequest) => Promise<void>;
  createApiError: (status: number, code: string, message: string) => Error;
}

export interface FriendsApi {
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

export function createFriendsApi(input: FriendsApiDependencies): FriendsApi {
  return {
    async fetchFriends(session) {
      const dto = await input.requestJson({
        method: "GET",
        path: "/friends",
        accessToken: session.accessToken,
      });
      return friendListFromResponse(dto);
    },

    async fetchFriendRequests(session) {
      const dto = await input.requestJson({
        method: "GET",
        path: "/friends/requests",
        accessToken: session.accessToken,
      });
      return friendRequestListFromResponse(dto);
    },

    async createFriendRequest(session, recipientUserId) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/friends/requests",
        accessToken: session.accessToken,
        body: { recipient_user_id: recipientUserId },
      });
      return friendRequestCreateFromResponse(dto);
    },

    async acceptFriendRequest(session, requestId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/friends/requests/${requestId}/accept`,
        accessToken: session.accessToken,
      });

      if (!dto || typeof dto !== "object" || (dto as { accepted?: unknown }).accepted !== true) {
        throw input.createApiError(
          500,
          "invalid_friend_accept_shape",
          "Unexpected friend accept response.",
        );
      }
    },

    async deleteFriendRequest(session, requestId) {
      await input.requestNoContent({
        method: "DELETE",
        path: `/friends/requests/${requestId}`,
        accessToken: session.accessToken,
      });
    },

    async removeFriend(session, friendUserId) {
      await input.requestNoContent({
        method: "DELETE",
        path: `/friends/${friendUserId}`,
        accessToken: session.accessToken,
      });
    },
  };
}