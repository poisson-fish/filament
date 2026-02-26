import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  attachmentFilenameFromInput,
  attachmentIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
  messageContentFromInput,
  messageIdFromInput,
  reactionEmojiFromInput,
  searchQueryFromInput,
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
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
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
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
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
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
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
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    const emoji = reactionEmojiFromInput("🔥");
    await expect(api.addMessageReaction(session, guildId, channelId, messageId, emoji)).resolves.toEqual({
      emoji,
      count: 1,
      reactedByMe: null,
      reactorUserIds: null,
    });
    await expect(api.removeMessageReaction(session, guildId, channelId, messageId, emoji)).resolves.toEqual({
      emoji,
      count: 0,
      reactedByMe: null,
      reactorUserIds: null,
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

  it("searchGuildMessages applies bounded query params and strict DTO parsing", async () => {
    const requestJson = vi.fn(async () => ({
      message_ids: ["01ARZ3NDEKTSV4RRFFQ69G5FB1"],
      messages: [
        {
          message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          content: "hello search",
          markdown_tokens: [],
          attachments: [],
          created_at_unix: 1_700_000_000,
        },
      ],
    }));

    const api = createMessagesApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await expect(
      api.searchGuildMessages(session, guildId, {
        query: searchQueryFromInput("hello"),
        limit: 25,
        channelId,
      }),
    ).resolves.toMatchObject({
      messageIds: ["01ARZ3NDEKTSV4RRFFQ69G5FB1"],
      messages: [{ messageId: "01ARZ3NDEKTSV4RRFFQ69G5FB1" }],
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/${guildId}/search?q=hello&limit=25&channel_id=${channelId}`,
      accessToken: session.accessToken,
    });
  });

  it("searchGuildMessages ignores out-of-range limit values", async () => {
    const requestJson = vi.fn(async () => ({ message_ids: [], messages: [] }));

    const api = createMessagesApi({
      requestJson,
      requestNoContent: vi.fn(async () => undefined),
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await api.searchGuildMessages(session, guildId, {
      query: searchQueryFromInput("hello"),
      limit: 500,
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/${guildId}/search?q=hello`,
      accessToken: session.accessToken,
    });
  });

  it("rebuild/reconcile search index delegate through bounded request primitives", async () => {
    const requestJson = vi.fn(async () => ({ upserted: 2, deleted: 1 }));
    const requestNoContent = vi.fn(async () => undefined);
    const api = createMessagesApi({
      requestJson,
      requestNoContent,
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await expect(api.rebuildGuildSearchIndex(session, guildId)).resolves.toBeUndefined();
    await expect(api.reconcileGuildSearchIndex(session, guildId)).resolves.toEqual({
      upserted: 2,
      deleted: 1,
    });

    expect(requestNoContent).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/search/rebuild`,
      accessToken: session.accessToken,
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/search/reconcile`,
      accessToken: session.accessToken,
    });
  });

  it("uploadChannelAttachment enforces size cap and uses body request primitive", async () => {
    const requestJsonWithBody = vi.fn(async () => ({
      attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB4",
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      owner_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
      filename: "avatar.png",
      mime_type: "image/png",
      size_bytes: 4,
      sha256_hex: "a".repeat(64),
    }));
    const api = createMessagesApi({
      requestJson: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      requestJsonWithBody,
      requestBinary: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    const file = new File([new Uint8Array([1, 2, 3, 4])], "avatar.png", { type: "image/png" });
    const filename = attachmentFilenameFromInput("avatar.png");
    await expect(
      api.uploadChannelAttachment(session, guildId, channelId, file, filename),
    ).resolves.toMatchObject({
      attachmentId: "01ARZ3NDEKTSV4RRFFQ69G5FB4",
      filename,
    });

    expect(requestJsonWithBody).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/channels/${channelId}/attachments?filename=avatar.png`,
      accessToken: session.accessToken,
      headers: { "content-type": "image/png" },
      body: file,
    });

    const oversizedFile = new File([new Uint8Array(25 * 1024 * 1024 + 1)], "large.bin", {
      type: "application/octet-stream",
    });
    await expect(
      api.uploadChannelAttachment(session, guildId, channelId, oversizedFile, attachmentFilenameFromInput("large.bin")),
    ).rejects.toMatchObject({ status: 400, code: "invalid_request" });
  });

  it("attachment download delegates bounded binary requests", async () => {
    const requestBinary = vi
      .fn()
      .mockResolvedValueOnce({ bytes: new Uint8Array([1]), mimeType: "image/png" })
      .mockResolvedValueOnce({ bytes: new Uint8Array([2]), mimeType: "image/png" });
    const attachmentId = attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB4");
    const api = createMessagesApi({
      requestJson: vi.fn(async () => null),
      requestNoContent: vi.fn(async () => undefined),
      requestJsonWithBody: vi.fn(async () => null),
      requestBinary,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
      isApiErrorCode: () => false,
    });

    await api.downloadChannelAttachment(session, guildId, channelId, attachmentId);
    await api.downloadChannelAttachmentPreview(session, guildId, channelId, attachmentId);

    expect(requestBinary).toHaveBeenNthCalledWith(1, {
      path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
      accessToken: session.accessToken,
    });
    expect(requestBinary).toHaveBeenNthCalledWith(2, {
      path: `/guilds/${guildId}/channels/${channelId}/attachments/${attachmentId}`,
      accessToken: session.accessToken,
      timeoutMs: 15_000,
      maxBytes: 12 * 1024 * 1024,
    });
  });
});
