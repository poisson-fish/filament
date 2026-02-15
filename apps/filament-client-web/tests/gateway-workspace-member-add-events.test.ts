import {
  decodeWorkspaceMemberAddGatewayEvent,
  isWorkspaceMemberAddGatewayEventType,
} from "../src/lib/gateway-workspace-member-add-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberAddGatewayEvent", () => {
  it("decodes valid workspace_member_add payload", () => {
    const result = decodeWorkspaceMemberAddGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role: "member",
      joined_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_member_add",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        role: "member",
        joinedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid workspace_member_add payload", () => {
    const result = decodeWorkspaceMemberAddGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role: "member",
      joined_at_unix: 0,
    });

    expect(result).toBeNull();
  });

  it("returns null for non-add event type", () => {
    const result = decodeWorkspaceMemberAddGatewayEvent("workspace_member_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role: "member",
      joined_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceMemberAddGatewayEventType", () => {
  it("classifies only workspace_member_add", () => {
    expect(isWorkspaceMemberAddGatewayEventType("workspace_member_add")).toBe(true);
    expect(isWorkspaceMemberAddGatewayEventType("workspace_member_update")).toBe(false);
  });
});