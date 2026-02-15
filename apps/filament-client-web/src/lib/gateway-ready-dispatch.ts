import {
  userIdFromInput,
} from "../domain/chat";

export interface ReadyPayload {
  userId: string;
}

export interface ReadyGatewayDispatchHandlers {
  onReady?: (payload: ReadyPayload) => void;
}

function parseReadyPayload(payload: unknown): ReadyPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.user_id !== "string") {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return { userId };
}

export function dispatchReadyGatewayEvent(
  type: string,
  payload: unknown,
  handlers: ReadyGatewayDispatchHandlers,
): boolean {
  if (type !== "ready") {
    return false;
  }

  const readyPayload = parseReadyPayload(payload);
  if (!readyPayload) {
    return true;
  }

  handlers.onReady?.(readyPayload);
  return true;
}