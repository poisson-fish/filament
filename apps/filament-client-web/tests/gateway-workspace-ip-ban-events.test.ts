import {
  decodeWorkspaceIpBanGatewayEvent,
  isWorkspaceIpBanGatewayEventType,
} from "../src/lib/gateway-workspace-ip-ban-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("decodeWorkspaceIpBanGatewayEvent", () => {
  it("exposes strict workspace ip-ban event type guard", () => {
    expect(isWorkspaceIpBanGatewayEventType("workspace_ip_ban_sync")).toBe(true);
    expect(isWorkspaceIpBanGatewayEventType("workspace_update")).toBe(false);
  });

  it("decodes valid workspace_ip_ban_sync payload", () => {
    const result = decodeWorkspaceIpBanGatewayEvent("workspace_ip_ban_sync", {
      guild_id: DEFAULT_GUILD_ID,
      summary: {
        action: "upsert",
        changed_count: 2,
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_ip_ban_sync",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        summary: {
          action: "upsert",
          changedCount: 2,
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid summary payload", () => {
    const result = decodeWorkspaceIpBanGatewayEvent("workspace_ip_ban_sync", {
      guild_id: DEFAULT_GUILD_ID,
      summary: {
        action: "remove",
        changed_count: -1,
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeWorkspaceIpBanGatewayEvent("workspace_unknown", {
      guild_id: DEFAULT_GUILD_ID,
    });

    expect(result).toBeNull();
  });
});