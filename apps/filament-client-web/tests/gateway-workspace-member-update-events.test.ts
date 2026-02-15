import {
  decodeWorkspaceMemberUpdateGatewayEvent,
  isWorkspaceMemberUpdateGatewayEventType,
} from "../src/lib/gateway-workspace-member-update-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberUpdateGatewayEvent", () => {
  it("decodes valid workspace_member_update payload", () => {
    const result = decodeWorkspaceMemberUpdateGatewayEvent("workspace_member_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        role: "moderator",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_member_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        updatedFields: {
          role: "moderator",
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed when role update field is missing", () => {
    const result = decodeWorkspaceMemberUpdateGatewayEvent("workspace_member_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      updated_fields: {},
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for non-update event type", () => {
    const result = decodeWorkspaceMemberUpdateGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        role: "moderator",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceMemberUpdateGatewayEventType", () => {
  it("classifies only workspace_member_update", () => {
    expect(isWorkspaceMemberUpdateGatewayEventType("workspace_member_update")).toBe(true);
    expect(isWorkspaceMemberUpdateGatewayEventType("workspace_member_add")).toBe(false);
  });
});
