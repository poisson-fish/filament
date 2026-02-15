import {
  decodeWorkspaceRoleReorderGatewayEvent,
  isWorkspaceRoleReorderGatewayEventType,
} from "../src/lib/gateway-workspace-role-reorder-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleReorderGatewayEvent", () => {
  it("decodes valid workspace_role_reorder payload", () => {
    const result = decodeWorkspaceRoleReorderGatewayEvent("workspace_role_reorder", {
      guild_id: DEFAULT_GUILD_ID,
      role_ids: [DEFAULT_ROLE_ID],
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_reorder",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        roleIds: [DEFAULT_ROLE_ID],
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for workspace_role_reorder payloads exceeding role id cap", () => {
    const roleIds = Array.from({ length: 65 }, (_, index) =>
      `01ARZ3NDEKTSV4RRFFQ69G5F${String(index).padStart(2, "0")}`,
    );

    const result = decodeWorkspaceRoleReorderGatewayEvent("workspace_role_reorder", {
      guild_id: DEFAULT_GUILD_ID,
      role_ids: roleIds,
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown workspace role reorder event type", () => {
    const result = decodeWorkspaceRoleReorderGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role_ids: [DEFAULT_ROLE_ID],
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleReorderGatewayEventType", () => {
  it("classifies only workspace role reorder event type", () => {
    expect(isWorkspaceRoleReorderGatewayEventType("workspace_role_reorder")).toBe(true);
    expect(isWorkspaceRoleReorderGatewayEventType("workspace_role_create")).toBe(false);
  });
});