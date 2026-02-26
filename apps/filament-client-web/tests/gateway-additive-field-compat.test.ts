import {
  decodeMessageGatewayEvent,
} from "../src/lib/gateway-message-events";
import {
  dispatchReadyGatewayEvent,
  dispatchSubscribedGatewayEvent,
} from "../src/lib/gateway-ready-dispatch";
import {
  decodeWorkspaceGatewayEvent,
} from "../src/lib/gateway-workspace-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_AUTHOR_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("gateway additive field compatibility", () => {
  it("accepts additive fields on ready and subscribed payloads", () => {
    const onReady = vi.fn();
    const onSubscribed = vi.fn();

    const readyHandled = dispatchReadyGatewayEvent(
      "ready",
      {
        user_id: DEFAULT_USER_ID,
        trace_context: {
          span_id: "span-123",
        },
      },
      { onReady },
    );
    const subscribedHandled = dispatchSubscribedGatewayEvent(
      "subscribed",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        feature_flag_snapshot: ["new-field"],
      },
      { onSubscribed },
    );

    expect(readyHandled).toBe(true);
    expect(subscribedHandled).toBe(true);
    expect(onReady).toHaveBeenCalledWith({ userId: DEFAULT_USER_ID });
    expect(onSubscribed).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
    });
  });

  it("accepts additive fields on message_create payloads", () => {
    const result = decodeMessageGatewayEvent("message_create", {
      message_id: DEFAULT_MESSAGE_ID,
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      author_id: DEFAULT_AUTHOR_ID,
      content: "hello",
      markdown_tokens: [{ type: "text", text: "hello" }],
      attachments: [],
      created_at_unix: 1710000001,
      reply_to_message_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      delivery_metadata: {
        via: "gateway",
      },
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
        reactions: [],
        createdAtUnix: 1710000001,
      },
    });
  });

  it("accepts additive fields on workspace_update payloads", () => {
    const result = decodeWorkspaceGatewayEvent("workspace_update", {
      guild_id: DEFAULT_GUILD_ID,
      updated_fields: {
        name: "Filament Workspace",
        experimental_label: "new-value",
      },
      updated_at_unix: 1710000001,
      schema_hint: 2,
    });

    expect(result).toEqual({
      type: "workspace_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        updatedFields: {
          name: "Filament Workspace",
          visibility: undefined,
        },
        updatedAtUnix: 1710000001,
      },
    });
  });
});
