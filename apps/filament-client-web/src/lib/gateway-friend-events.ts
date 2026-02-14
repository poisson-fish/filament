import {
  friendRequestIdFromInput,
  userIdFromInput,
} from "../domain/chat";
import type {
  FriendRemovePayload,
  FriendRequestCreatePayload,
  FriendRequestDeletePayload,
  FriendRequestUpdatePayload,
} from "./gateway";

const MAX_FRIEND_USERNAME_LENGTH = 64;

type FriendGatewayEvent =
  | {
      type: "friend_request_create";
      payload: FriendRequestCreatePayload;
    }
  | {
      type: "friend_request_update";
      payload: FriendRequestUpdatePayload;
    }
  | {
      type: "friend_request_delete";
      payload: FriendRequestDeletePayload;
    }
  | {
      type: "friend_remove";
      payload: FriendRemovePayload;
    };

type FriendGatewayEventType = FriendGatewayEvent["type"];
type FriendEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseFriendRequestCreatePayload(
  payload: unknown,
): FriendRequestCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.request_id !== "string" ||
    typeof value.sender_user_id !== "string" ||
    typeof value.sender_username !== "string" ||
    value.sender_username.length === 0 ||
    value.sender_username.length > MAX_FRIEND_USERNAME_LENGTH ||
    typeof value.recipient_user_id !== "string" ||
    typeof value.recipient_username !== "string" ||
    value.recipient_username.length === 0 ||
    value.recipient_username.length > MAX_FRIEND_USERNAME_LENGTH ||
    typeof value.created_at_unix !== "number" ||
    !Number.isSafeInteger(value.created_at_unix) ||
    value.created_at_unix < 1
  ) {
    return null;
  }

  try {
    friendRequestIdFromInput(value.request_id);
  } catch {
    return null;
  }

  let senderUserId: string;
  let recipientUserId: string;
  try {
    senderUserId = userIdFromInput(value.sender_user_id);
    recipientUserId = userIdFromInput(value.recipient_user_id);
  } catch {
    return null;
  }

  return {
    requestId: value.request_id,
    senderUserId,
    senderUsername: value.sender_username,
    recipientUserId,
    recipientUsername: value.recipient_username,
    createdAtUnix: value.created_at_unix,
  };
}

function parseFriendRequestUpdatePayload(
  payload: unknown,
): FriendRequestUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.request_id !== "string" ||
    value.state !== "accepted" ||
    typeof value.user_id !== "string" ||
    typeof value.friend_user_id !== "string" ||
    typeof value.friend_username !== "string" ||
    value.friend_username.length === 0 ||
    value.friend_username.length > MAX_FRIEND_USERNAME_LENGTH ||
    typeof value.friendship_created_at_unix !== "number" ||
    !Number.isSafeInteger(value.friendship_created_at_unix) ||
    value.friendship_created_at_unix < 1 ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  try {
    friendRequestIdFromInput(value.request_id);
  } catch {
    return null;
  }

  let userId: string;
  let friendUserId: string;
  try {
    userId = userIdFromInput(value.user_id);
    friendUserId = userIdFromInput(value.friend_user_id);
  } catch {
    return null;
  }

  return {
    requestId: value.request_id,
    state: "accepted",
    userId,
    friendUserId,
    friendUsername: value.friend_username,
    friendshipCreatedAtUnix: value.friendship_created_at_unix,
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseFriendRequestDeletePayload(
  payload: unknown,
): FriendRequestDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.request_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }
  try {
    friendRequestIdFromInput(value.request_id);
  } catch {
    return null;
  }
  return {
    requestId: value.request_id,
    deletedAtUnix: value.deleted_at_unix,
  };
}

function parseFriendRemovePayload(payload: unknown): FriendRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.user_id !== "string" ||
    typeof value.friend_user_id !== "string" ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
  ) {
    return null;
  }

  let userId: string;
  let friendUserId: string;
  try {
    userId = userIdFromInput(value.user_id);
    friendUserId = userIdFromInput(value.friend_user_id);
  } catch {
    return null;
  }

  return {
    userId,
    friendUserId,
    removedAtUnix: value.removed_at_unix,
  };
}

const FRIEND_EVENT_DECODERS: {
  [K in FriendGatewayEventType]: FriendEventDecoder<Extract<FriendGatewayEvent, { type: K }>["payload"]>;
} = {
  friend_request_create: parseFriendRequestCreatePayload,
  friend_request_update: parseFriendRequestUpdatePayload,
  friend_request_delete: parseFriendRequestDeletePayload,
  friend_remove: parseFriendRemovePayload,
};

function isFriendGatewayEventType(value: string): value is FriendGatewayEventType {
  return value in FRIEND_EVENT_DECODERS;
}

export function decodeFriendGatewayEvent(
  type: string,
  payload: unknown,
): FriendGatewayEvent | null {
  if (!isFriendGatewayEventType(type)) {
    return null;
  }

  if (type === "friend_request_create") {
    const parsedPayload = FRIEND_EVENT_DECODERS.friend_request_create(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "friend_request_update") {
    const parsedPayload = FRIEND_EVENT_DECODERS.friend_request_update(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "friend_request_delete") {
    const parsedPayload = FRIEND_EVENT_DECODERS.friend_request_delete(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  const parsedPayload = FRIEND_EVENT_DECODERS.friend_remove(payload);
  if (!parsedPayload) {
    return null;
  }
  return {
    type,
    payload: parsedPayload,
  };
}