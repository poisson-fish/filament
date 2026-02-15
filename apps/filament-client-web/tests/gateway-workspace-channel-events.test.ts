import {
  decodeWorkspaceChannelGatewayEvent,
  isWorkspaceChannelGatewayEventType,
} from "../src/lib/gateway-workspace-channel-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("decodeWorkspaceChannelGatewayEvent", () => {
  it("exposes strict workspace channel event type guard", () => {
    expect(isWorkspaceChannelGatewayEventType("channel_create")).toBe(true);
    expect(isWorkspaceChannelGatewayEventType("workspace_update")).toBe(false);
  });

  it("decodes valid channel_create payload", () => {
    const result = decodeWorkspaceChannelGatewayEvent("channel_create", {
      guild_id: DEFAULT_GUILD_ID,
      channel: {
        channel_id: DEFAULT_CHANNEL_ID,
        name: "general",
        kind: "text",
      },
    });

    expect(result).toEqual({
      type: "channel_create",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channel: {
          channelId: DEFAULT_CHANNEL_ID,
          name: "general",
          kind: "text",
        },
      },
    });
  });

  it("fails closed for invalid channel_create payload", () => {
    const result = decodeWorkspaceChannelGatewayEvent("channel_create", {
      guild_id: DEFAULT_GUILD_ID,
      channel: {
        channel_id: DEFAULT_CHANNEL_ID,
        name: "",
        kind: "text",
      },
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeWorkspaceChannelGatewayEvent("workspace_unknown", {
      guild_id: DEFAULT_GUILD_ID,
    });

    expect(result).toBeNull();
  });
});