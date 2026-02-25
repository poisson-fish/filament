import {
  decodeWorkspaceRoleCreateGatewayEvent,
  isWorkspaceRoleCreateGatewayEventType,
} from "../src/lib/gateway-workspace-role-create-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleCreateGatewayEvent", () => {
  it("decodes valid workspace_role_create payload", () => {
    const result = decodeWorkspaceRoleCreateGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 2,
        is_system: false,
        permissions: ["manage_roles", "create_message"],
        color_hex: "#00aaff",
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
          colorHex: "#00AAFF",
        },
      },
    });
  });

  it("decodes nullable workspace role color payloads", () => {
    const result = decodeWorkspaceRoleCreateGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 2,
        is_system: false,
        permissions: ["manage_roles"],
        color_hex: null,
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
          permissions: ["manage_roles"],
          colorHex: null,
        },
      },
    });
  });

  it("fails closed for invalid workspace_role_create payload", () => {
    const result = decodeWorkspaceRoleCreateGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 0,
        is_system: false,
        permissions: ["manage_roles"],
      },
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid workspace role create color values", () => {
    const result = decodeWorkspaceRoleCreateGatewayEvent("workspace_role_create", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 2,
        is_system: false,
        permissions: ["manage_roles"],
        color_hex: "#12Z45G",
      },
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown workspace role create event type", () => {
    const result = decodeWorkspaceRoleCreateGatewayEvent("workspace_role_update", {
      guild_id: DEFAULT_GUILD_ID,
      role: {
        role_id: DEFAULT_ROLE_ID,
        name: "moderator",
        position: 2,
        is_system: false,
        permissions: ["manage_roles"],
      },
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleCreateGatewayEventType", () => {
  it("classifies only workspace role create event type", () => {
    expect(isWorkspaceRoleCreateGatewayEventType("workspace_role_create")).toBe(true);
    expect(isWorkspaceRoleCreateGatewayEventType("workspace_role_update")).toBe(false);
  });
});
