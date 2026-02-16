import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  attachmentFromResponse,
  channelIdFromInput,
  guildIdFromInput,
  messageContentFromInput,
  messageIdFromInput,
  messageFromResponse,
  reactionFromResponse,
  searchQueryFromInput,
  searchReconcileFromResponse,
  searchResultsFromResponse,
} from "../src/domain/chat";
import type { MessagesApi } from "../src/lib/api-messages";
import { createMessagesClient } from "../src/lib/api-messages-client";

describe("api-messages-client", () => {
  function createSession() {
    return {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
  }

  function createMessagesApiStub(overrides?: Partial<MessagesApi>): MessagesApi {
    const defaultMessage = messageFromResponse({
      message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
      content: "hello",
      markdown_tokens: [],
      attachments: [],
      created_at_unix: 1_700_000_000,
    });
    const defaultSearchResults = searchResultsFromResponse({
      message_ids: ["01ARZ3NDEKTSV4RRFFQ69G5FB1"],
      messages: [
        {
          message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          content: "hello",
          markdown_tokens: [],
          attachments: [],
          created_at_unix: 1_700_000_000,
        },
      ],
    });

    const api: MessagesApi = {
      fetchChannelMessages: vi.fn(async () => ({ messages: [], nextBefore: null })),
      createChannelMessage: vi.fn(async () => defaultMessage),
      editChannelMessage: vi.fn(async () => defaultMessage),
      deleteChannelMessage: vi.fn(async () => undefined),
      addMessageReaction: vi.fn(async () => reactionFromResponse({ emoji: "🔥", count: 1 })),
      removeMessageReaction: vi.fn(async () => reactionFromResponse({ emoji: "🔥", count: 0 })),
      searchGuildMessages: vi.fn(async () => defaultSearchResults),
      rebuildGuildSearchIndex: vi.fn(async () => undefined),
      reconcileGuildSearchIndex: vi.fn(async () =>
        searchReconcileFromResponse({ upserted: 0, deleted: 0 }),
      ),
      uploadChannelAttachment: vi.fn(async () =>
        attachmentFromResponse({
          attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB3",
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          owner_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          filename: "test.txt",
          mime_type: "text/plain",
          size_bytes: 4,
          sha256_hex: "a".repeat(64),
          uploaded_at_unix: 1_700_000_000,
        }),
      ),
      downloadChannelAttachment: vi.fn(async () => ({ bytes: new Uint8Array(), mimeType: null })),
      downloadChannelAttachmentPreview: vi.fn(async () => ({
        bytes: new Uint8Array(),
        mimeType: null,
      })),
      deleteChannelAttachment: vi.fn(async () => undefined),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates createChannelMessage through messages API", async () => {
    const createChannelMessage = vi.fn(async () =>
      messageFromResponse({
        message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
        guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
        author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
        content: "delegated",
        markdown_tokens: [],
        attachments: [],
        created_at_unix: 1_700_000_000,
      }),
    );
    const client = createMessagesClient({
      messagesApi: createMessagesApiStub({ createChannelMessage }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");

    await client.createChannelMessage(session, guildId, channelId, {
      content: messageContentFromInput("delegated"),
    });

    expect(createChannelMessage).toHaveBeenCalledWith(session, guildId, channelId, {
      content: "delegated",
    });
  });

  it("delegates searchGuildMessages and returns upstream value", async () => {
    const expectedSearch = searchResultsFromResponse({
      message_ids: ["01ARZ3NDEKTSV4RRFFQ69G5FB1"],
      messages: [
        {
          message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
          author_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          content: "search-result",
          markdown_tokens: [],
          attachments: [],
          created_at_unix: 1_700_000_000,
        },
      ],
    });
    const searchGuildMessages = vi.fn(async () => expectedSearch);
    const client = createMessagesClient({
      messagesApi: createMessagesApiStub({ searchGuildMessages }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await expect(
      client.searchGuildMessages(session, guildId, {
        query: searchQueryFromInput("search-result"),
      }),
    ).resolves.toBe(expectedSearch);
    expect(searchGuildMessages).toHaveBeenCalledWith(session, guildId, {
      query: searchQueryFromInput("search-result"),
    });
  });

  it("delegates deleteChannelMessage", async () => {
    const deleteChannelMessage = vi.fn(async () => undefined);
    const client = createMessagesClient({
      messagesApi: createMessagesApiStub({ deleteChannelMessage }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
    const messageId = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");

    await client.deleteChannelMessage(session, guildId, channelId, messageId);

    expect(deleteChannelMessage).toHaveBeenCalledWith(session, guildId, channelId, messageId);
  });
});
