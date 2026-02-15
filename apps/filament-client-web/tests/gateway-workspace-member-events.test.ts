import {
  decodeWorkspaceMemberGatewayEvent,
  isWorkspaceMemberGatewayEventType,
} from "../src/lib/gateway-workspace-member-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceMemberGatewayEvent", () => {
  it("decodes valid workspace_member_add payload via aggregate member decoder", () => {
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

  it("fails closed for invalid workspace_member_add payload delegated through aggregate member decoder", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role: "member",
      joined_at_unix: 0,
    });

    expect(result).toBeNull();
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

  it("decodes valid workspace_member_update payload via aggregate member decoder", () => {
    const result = decodeWorkspaceMemberGatewayEvent("workspace_member_update", {
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

describe("isWorkspaceMemberGatewayEventType", () => {
  it("classifies only workspace member event types", () => {
    expect(isWorkspaceMemberGatewayEventType("workspace_member_add")).toBe(true);
    expect(isWorkspaceMemberGatewayEventType("workspace_member_update")).toBe(true);
    expect(isWorkspaceMemberGatewayEventType("workspace_member_remove")).toBe(true);
    expect(isWorkspaceMemberGatewayEventType("workspace_member_ban")).toBe(true);
    expect(isWorkspaceMemberGatewayEventType("workspace_role_create")).toBe(false);
  });
});
