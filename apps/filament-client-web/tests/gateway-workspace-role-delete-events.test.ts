import {
  decodeWorkspaceRoleDeleteGatewayEvent,
  isWorkspaceRoleDeleteGatewayEventType,
} from "../src/lib/gateway-workspace-role-delete-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleDeleteGatewayEvent", () => {
  it("decodes valid workspace_role_delete payload", () => {
    const result = decodeWorkspaceRoleDeleteGatewayEvent("workspace_role_delete", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      deleted_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_delete",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        roleId: DEFAULT_ROLE_ID,
        deletedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid workspace_role_delete payload", () => {
    const result = decodeWorkspaceRoleDeleteGatewayEvent("workspace_role_delete", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      deleted_at_unix: 0,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown workspace role delete event type", () => {
    const result = decodeWorkspaceRoleDeleteGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      deleted_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleDeleteGatewayEventType", () => {
  it("classifies only workspace role delete event type", () => {
    expect(isWorkspaceRoleDeleteGatewayEventType("workspace_role_delete")).toBe(true);
    expect(isWorkspaceRoleDeleteGatewayEventType("workspace_role_update")).toBe(false);
  });
});