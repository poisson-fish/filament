import { decodeWorkspaceMemberGatewayEvent } from "../src/lib/gateway-workspace-member-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberGatewayEvent", () => {
  it("decodes valid workspace_member_add payload", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_add", {
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

  it("fails closed for workspace_member_update payload without updatable fields", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      updated_fields: {},
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for workspace_member_remove payload with invalid reason", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      reason: "timeout",
      removed_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown member event type", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
    });

    expect(result).toBeNull();
  });
});
