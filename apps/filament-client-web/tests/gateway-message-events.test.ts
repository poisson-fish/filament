import {
  decodeMessageGatewayEvent,
} from "../src/lib/gateway-message-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_AUTHOR_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeMessageGatewayEvent", () => {
  it("decodes valid message_create payload", () => {
    const result = decodeMessageGatewayEvent("message_create", {
      message_id: DEFAULT_MESSAGE_ID,
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      author_id: DEFAULT_AUTHOR_ID,
      content: "hello",
      markdown_tokens: [{ type: "text", text: "hello" }],
      attachments: [],
      created_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "message_create",
      payload: {
        messageId: DEFAULT_MESSAGE_ID,
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        authorId: DEFAULT_AUTHOR_ID,
        content: "hello",
        markdownTokens: [{ type: "text", text: "hello" }],
        attachments: [],
        createdAtUnix: 1710000001,
      },
    });
  });

  it("decodes valid message_update payload", () => {
    const result = decodeMessageGatewayEvent("message_update", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      message_id: DEFAULT_MESSAGE_ID,
      updated_fields: {
        content: "updated",
      },
      updated_at_unix: 1710000002,
    });

    expect(result).toEqual({
      type: "message_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        messageId: DEFAULT_MESSAGE_ID,
        updatedFields: {
          content: "updated",
          markdownTokens: undefined,
        },
        updatedAtUnix: 1710000002,
      },
    });
  });

  it("fails closed for invalid message_delete payload", () => {
    const result = decodeMessageGatewayEvent("message_delete", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      message_id: "not-a-ulid",
      deleted_at_unix: 1710000003,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid message_reaction payload", () => {
    const result = decodeMessageGatewayEvent("message_reaction", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      message_id: DEFAULT_MESSAGE_ID,
      emoji: "👍",
      count: -1,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeMessageGatewayEvent("message_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      message_id: DEFAULT_MESSAGE_ID,
      deleted_at_unix: 1710000003,
    });

    expect(result).toBeNull();
  });
});
