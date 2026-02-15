import {
  dispatchWorkspaceGatewayEvent,
} from "../src/lib/gateway-workspace-dispatch";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("dispatchWorkspaceGatewayEvent", () => {
  it("dispatches decoded workspace events to matching handlers", () => {
    const onChannelCreate = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "channel_create",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel: {
          channel_id: DEFAULT_CHANNEL_ID,
          name: "general",
          kind: "text",
        },
      },
      { onChannelCreate },
    );

    expect(handled).toBe(true);
    expect(onChannelCreate).toHaveBeenCalledTimes(1);
    expect(onChannelCreate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channel: {
        channelId: DEFAULT_CHANNEL_ID,
        name: "general",
        kind: "text",
      },
    });
  });

  it("fails closed for known workspace event types with invalid payloads", () => {
    const onWorkspaceMemberAdd = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "workspace_member_add",
      {
        guild_id: DEFAULT_GUILD_ID,
        user_id: DEFAULT_USER_ID,
        role: "",
        joined_at_unix: 1710000001,
      },
      { onWorkspaceMemberAdd },
    );

    expect(handled).toBe(true);
    expect(onWorkspaceMemberAdd).not.toHaveBeenCalled();
  });

  it("returns false for non-workspace event types", () => {
    const onWorkspaceIpBanSync = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "profile_update",
      {},
      { onWorkspaceIpBanSync },
    );

    expect(handled).toBe(false);
    expect(onWorkspaceIpBanSync).not.toHaveBeenCalled();
  });
});