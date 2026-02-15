import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  attachmentIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
  messageContentFromInput,
  messageIdFromInput,
  reactionEmojiFromInput,
} from "../src/domain/chat";
import { createMessagesApi } from "../src/lib/api-messages";

class MockApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "MockApiError";
    this.status = status;
    this.code = code;
  }
}

describe("api-messages", () => {
  const session = {
    accessToken: accessTokenFromInput("A".repeat(64)),
    refreshToken: refreshTokenFromInput("B".repeat(64)),
    expiresAtUnix: 2_000_000_000,
  };
  const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
  const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
  const messageId = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetchChannelMessages applies bounded query params and strict DTO parsing", async () => {
    const requestJson = vi.fn(async () => ({
      messages: [
        {
          message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          content: "hello world",
          markdown_tokens: [],
          attachments: [],
          created_at_unix: 1_700_000_000,
        },
      ],
      next_before: "01ARZ3NDEKTSV4RRFFQ69G5FB3",
    }));

    const api = createMessagesApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await expect(
      api.fetchChannelMessages(session, guildId, channelId, {
        limit: 25,
        before: messageId,
      }),
    ).resolves.toMatchObject({
      nextBefore: "01ARZ3NDEKTSV4RRFFQ69G5FB3",
      messages: [{ messageId: "01ARZ3NDEKTSV4RRFFQ69G5FB1" }],
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/${guildId}/channels/${channelId}/messages?limit=25&before=${messageId}`,
      accessToken: session.accessToken,
    });
  });

  it("createChannelMessage maps invalid_json with attachments to protocol_mismatch", async () => {
    const api = createMessagesApi({
      requestJson: vi.fn(async () => {
        throw new MockApiError(500, "invalid_json", "Malformed server response.");
      }),
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: (error, code) => error instanceof MockApiError && error.code === code,
    });

    await expect(
      api.createChannelMessage(session, guildId, channelId, {
        content: messageContentFromInput("hello"),
        attachmentIds: [attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB4")],
      }),
    ).rejects.toMatchObject({
      status: 400,
      code: "protocol_mismatch",
    });
  });

  it("deleteChannelMessage delegates to no-content request primitive", async () => {
    const requestNoContent = vi.fn(async () => undefined);
    const api = createMessagesApi({
      requestJson: vi.fn(async () => null),
      requestNoContent,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await api.deleteChannelMessage(session, guildId, channelId, messageId);

    expect(requestNoContent).toHaveBeenCalledWith({
      method: "DELETE",
      path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}`,
      accessToken: session.accessToken,
    });
  });

  it("add/remove reaction delegate to strict reaction DTO parsing", async () => {
    const requestJson = vi
      .fn()
      .mockResolvedValueOnce({ emoji: "🔥", count: 1 })
      .mockResolvedValueOnce({ emoji: "🔥", count: 0 });

    const api = createMessagesApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    const emoji = reactionEmojiFromInput("🔥");
    await expect(api.addMessageReaction(session, guildId, channelId, messageId, emoji)).resolves.toEqual({
      emoji,
      count: 1,
    });
    await expect(api.removeMessageReaction(session, guildId, channelId, messageId, emoji)).resolves.toEqual({
      emoji,
      count: 0,
    });

    expect(requestJson).toHaveBeenNthCalledWith(1, {
      method: "POST",
      path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      accessToken: session.accessToken,
    });
    expect(requestJson).toHaveBeenNthCalledWith(2, {
      method: "DELETE",
      path: `/guilds/${guildId}/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      accessToken: session.accessToken,
    });
  });
});
