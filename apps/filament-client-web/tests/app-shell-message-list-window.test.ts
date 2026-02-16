import { describe, expect, it } from "vitest";
import {
  DEFAULT_MESSAGE_LIST_RENDER_WINDOW_SIZE,
  MAX_MESSAGE_LIST_RENDER_WINDOW_SIZE,
  resolveMessageListRenderWindow,
} from "../src/features/app-shell/components/messages/message-list-window";

describe("app shell message list render window", () => {
  it("returns an empty range for empty or invalid message counts", () => {
    expect(
      resolveMessageListRenderWindow({
        messageCount: 0,
      }),
    ).toEqual({ startIndex: 0, endIndex: 0 });

    expect(
      resolveMessageListRenderWindow({
        messageCount: -4,
      }),
    ).toEqual({ startIndex: 0, endIndex: 0 });
  });

  it("returns the full range when messages are within the render window", () => {
    expect(
      resolveMessageListRenderWindow({
        messageCount: 12,
        maxRenderedMessages: 20,
      }),
    ).toEqual({ startIndex: 0, endIndex: 12 });
  });

  it("returns the trailing bounded window when messages exceed the window", () => {
    expect(
      resolveMessageListRenderWindow({
        messageCount: 1_000,
        maxRenderedMessages: 300,
      }),
    ).toEqual({ startIndex: 700, endIndex: 1_000 });
  });

  it("fails closed to defaults and hard cap for invalid max window input", () => {
    expect(
      resolveMessageListRenderWindow({
        messageCount: 1_000,
        maxRenderedMessages: 0,
      }),
    ).toEqual({
      startIndex: 1_000 - DEFAULT_MESSAGE_LIST_RENDER_WINDOW_SIZE,
      endIndex: 1_000,
    });

    expect(
      resolveMessageListRenderWindow({
        messageCount: 1_000,
        maxRenderedMessages: MAX_MESSAGE_LIST_RENDER_WINDOW_SIZE + 50,
      }),
    ).toEqual({
      startIndex: 1_000 - MAX_MESSAGE_LIST_RENDER_WINDOW_SIZE,
      endIndex: 1_000,
    });
  });
});
