import {
  decodeWorkspaceMemberBanGatewayEvent,
  isWorkspaceMemberBanGatewayEventType,
} from "../src/lib/gateway-workspace-member-ban-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberBanGatewayEvent", () => {
  it("decodes valid workspace_member_ban payload", () => {
    const result = decodeWorkspaceMemberBanGatewayEvent("workspace_member_ban", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      banned_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_member_ban",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        bannedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid workspace_member_ban payload", () => {
    const result = decodeWorkspaceMemberBanGatewayEvent("workspace_member_ban", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      banned_at_unix: 0,
    });

    expect(result).toBeNull();
  });

  it("returns null for non-ban event type", () => {
    const result = decodeWorkspaceMemberBanGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      banned_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceMemberBanGatewayEventType", () => {
  it("classifies only workspace_member_ban", () => {
    expect(isWorkspaceMemberBanGatewayEventType("workspace_member_ban")).toBe(true);
    expect(isWorkspaceMemberBanGatewayEventType("workspace_member_add")).toBe(false);
  });
});
