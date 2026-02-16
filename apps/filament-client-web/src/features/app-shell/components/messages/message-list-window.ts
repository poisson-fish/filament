export const DEFAULT_MESSAGE_LIST_RENDER_WINDOW_SIZE = 240;
export const MAX_MESSAGE_LIST_RENDER_WINDOW_SIZE = 600;

export interface MessageListRenderWindow {
  startIndex: number;
  endIndex: number;
}

export interface ResolveMessageListRenderWindowInput {
  messageCount: number;
  maxRenderedMessages?: number;
}

function sanitizePositiveInteger(
  input: number | undefined,
  fallback: number,
  maximum: number,
): number {
  if (!Number.isInteger(input) || (input ?? 0) < 1) {
    return fallback;
  }
  return Math.min(input as number, maximum);
}

export function resolveMessageListRenderWindow(
  input: ResolveMessageListRenderWindowInput,
): MessageListRenderWindow {
  const messageCount =
    Number.isInteger(input.messageCount) && input.messageCount > 0
      ? input.messageCount
      : 0;

  if (messageCount === 0) {
    return { startIndex: 0, endIndex: 0 };
  }

  const maxRenderedMessages = sanitizePositiveInteger(
    input.maxRenderedMessages,
    DEFAULT_MESSAGE_LIST_RENDER_WINDOW_SIZE,
    MAX_MESSAGE_LIST_RENDER_WINDOW_SIZE,
  );

  if (messageCount <= maxRenderedMessages) {
    return {
      startIndex: 0,
      endIndex: messageCount,
    };
  }

  return {
    startIndex: messageCount - maxRenderedMessages,
    endIndex: messageCount,
  };
}
