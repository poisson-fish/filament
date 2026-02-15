import {
  dispatchMessageGatewayEvent,
} from "../src/lib/gateway-message-dispatch";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_AUTHOR_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("dispatchMessageGatewayEvent", () => {
  it("dispatches decoded message events to matching handlers", () => {
    const onMessageCreate = vi.fn();

    const handled = dispatchMessageGatewayEvent(
      "message_create",
      {
        message_id: DEFAULT_MESSAGE_ID,
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        author_id: DEFAULT_AUTHOR_ID,
        content: "hello",
        markdown_tokens: [
          {
            type: "text",
            text: "hello",
          },
        ],
        attachments: [],
        created_at_unix: 1710000001,
      },
      { onMessageCreate },
    );

    expect(handled).toBe(true);
    expect(onMessageCreate).toHaveBeenCalledTimes(1);
    expect(onMessageCreate).toHaveBeenCalledWith({
      messageId: DEFAULT_MESSAGE_ID,
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      authorId: DEFAULT_AUTHOR_ID,
      content: "hello",
      markdownTokens: [
        {
          type: "text",
          text: "hello",
        },
      ],
      attachments: [],
      createdAtUnix: 1710000001,
    });
  });

  it("fails closed for known message types with invalid payloads", () => {
    const onMessageUpdate = vi.fn();

    const handled = dispatchMessageGatewayEvent(
      "message_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        message_id: DEFAULT_MESSAGE_ID,
        updated_fields: {},
        updated_at_unix: 1710000010,
      },
      { onMessageUpdate },
    );

    expect(handled).toBe(true);
    expect(onMessageUpdate).not.toHaveBeenCalled();
  });

  it("returns false for non-message event types", () => {
    const onMessageDelete = vi.fn();

    const handled = dispatchMessageGatewayEvent(
      "profile_update",
      {},
      { onMessageDelete },
    );

    expect(handled).toBe(false);
    expect(onMessageDelete).not.toHaveBeenCalled();
  });
});