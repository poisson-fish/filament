import { decodeWorkspaceRoleGatewayEvent } from "../src/lib/gateway-workspace-role-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleGatewayEvent", () => {
  it("decodes role-assignment payloads via aggregate role decoder", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_assignment_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      assigned_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_assignment_add",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        roleId: DEFAULT_ROLE_ID,
        assignedAtUnix: 1710000001,
      },
    });
  });

  it("decodes valid workspace_role_create payload", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 2,
        is_system: false,
        permissions: ["manage_roles", "create_message"],
      },
    });

    expect(result).toEqual({
      type: "workspace_role_create",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        role: {
          roleId: DEFAULT_ROLE_ID,
          name: "moderator",
          position: 2,
          isSystem: false,
          permissions: ["manage_roles", "create_message"],
        },
      },
    });
  });

  it("fails closed for workspace_role_reorder payloads exceeding the role id cap", () => {
    const roleIds = Array.from({ length: 65 }, (_, index) =>
      `01ARZ3NDEKTSV4RRFFQ69G5F${String(index).padStart(2, "0")}`,
    );

    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_reorder", {
      guild_id: DEFAULT_GUILD_ID,
      role_ids: roleIds,
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("decodes workspace_role_reorder payloads via aggregate role decoder", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_reorder", {
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

  it("decodes workspace_role_delete payloads via aggregate role decoder", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_delete", {
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

  it("decodes workspace_role_update payloads via aggregate role decoder", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {
        name: "moderator",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        roleId: DEFAULT_ROLE_ID,
        updatedFields: {
          name: "moderator",
          permissions: undefined,
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for workspace_role_update payload without any updatable fields", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {},
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown role event type", () => {
    const result = decodeWorkspaceRoleGatewayEvent("workspace_role_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
    });

    expect(result).toBeNull();
  });
});
