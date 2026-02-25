import {
  decodeWorkspaceRoleUpdateGatewayEvent,
  isWorkspaceRoleUpdateGatewayEventType,
} from "../src/lib/gateway-workspace-role-update-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleUpdateGatewayEvent", () => {
  it("decodes valid workspace_role_update payload", () => {
    const result = decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {
        name: "moderator",
        permissions: ["manage_roles", "create_message"],
        color_hex: "#3366cc",
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
          permissions: ["manage_roles", "create_message"],
          colorHex: "#3366CC",
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("decodes null color updates for workspace_role_update payload", () => {
    const result = decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {
        color_hex: null,
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        roleId: DEFAULT_ROLE_ID,
        updatedFields: {
          colorHex: null,
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for payloads without updatable fields", () => {
    const result = decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {},
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown workspace role update event type", () => {
    const result = decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {
        name: "moderator",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid workspace role update color values", () => {
    const result = decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role_id: DEFAULT_ROLE_ID,
      updated_fields: {
        color_hex: "#ZZ11FF",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleUpdateGatewayEventType", () => {
  it("classifies only workspace role update event type", () => {
    expect(isWorkspaceRoleUpdateGatewayEventType("workspace_role_update")).toBe(true);
    expect(isWorkspaceRoleUpdateGatewayEventType("workspace_role_delete")).toBe(false);
  });
});
