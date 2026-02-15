import {
  decodeWorkspaceMemberRemoveGatewayEvent,
  isWorkspaceMemberRemoveGatewayEventType,
} from "../src/lib/gateway-workspace-member-remove-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberRemoveGatewayEvent", () => {
  it("decodes valid workspace_member_remove payload", () => {
    const result = decodeWorkspaceMemberRemoveGatewayEvent("workspace_member_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      reason: "kick",
      removed_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_member_remove",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        reason: "kick",
        removedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid workspace_member_remove payload", () => {
    const result = decodeWorkspaceMemberRemoveGatewayEvent("workspace_member_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      reason: "timeout",
      removed_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for non-remove event type", () => {
    const result = decodeWorkspaceMemberRemoveGatewayEvent("workspace_member_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      reason: "kick",
      removed_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceMemberRemoveGatewayEventType", () => {
  it("classifies only workspace_member_remove", () => {
    expect(isWorkspaceMemberRemoveGatewayEventType("workspace_member_remove")).toBe(true);
    expect(isWorkspaceMemberRemoveGatewayEventType("workspace_member_update")).toBe(false);
  });
});