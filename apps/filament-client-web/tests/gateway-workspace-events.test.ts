import {
  decodeWorkspaceGatewayEvent,
  isWorkspaceGatewayEventType,
} from "../src/lib/gateway-workspace-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("decodeWorkspaceGatewayEvent", () => {
  it("exposes strict workspace event type guard from decoder registries", () => {
    expect(isWorkspaceGatewayEventType("channel_create")).toBe(true);
    expect(isWorkspaceGatewayEventType("workspace_role_create")).toBe(true);
    expect(isWorkspaceGatewayEventType("workspace_member_add")).toBe(true);
    expect(isWorkspaceGatewayEventType("workspace_ip_ban_sync")).toBe(true);
    expect(isWorkspaceGatewayEventType("workspace_channel_override_update")).toBe(true);
    expect(isWorkspaceGatewayEventType("profile_update")).toBe(false);
  });

  it("decodes valid channel_create payload", () => {
    const result = decodeWorkspaceGatewayEvent("channel_create", {
      guild_id: DEFAULT_GUILD_ID,
      channel: {
        channel_id: DEFAULT_CHANNEL_ID,
        name: "general",
        kind: "text",
      },
    });

    expect(result).toEqual({
      type: "channel_create",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channel: {
          channelId: DEFAULT_CHANNEL_ID,
          name: "general",
          kind: "text",
        },
      },
    });
  });

  it("decodes valid workspace_update payload through delegated registry", () => {
    const result = decodeWorkspaceGatewayEvent("workspace_update", {
      guild_id: DEFAULT_GUILD_ID,
      updated_fields: {
        name: "Filament Workspace",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        updatedFields: {
          name: "Filament Workspace",
          visibility: undefined,
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid workspace_member_add payload", () => {
    const result = decodeWorkspaceGatewayEvent("workspace_member_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role: "",
      joined_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for workspace_role_reorder payloads exceeding the role id cap", () => {
    const roleIds = Array.from({ length: 65 }, (_, index) =>
      `01ARZ3NDEKTSV4RRFFQ69G5F${String(index).padStart(2, "0")}`,
    );

    const result = decodeWorkspaceGatewayEvent("workspace_role_reorder", {
      guild_id: DEFAULT_GUILD_ID,
      role_ids: roleIds,
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid workspace_ip_ban_sync summary payload", () => {
    const result = decodeWorkspaceGatewayEvent("workspace_ip_ban_sync", {
      guild_id: DEFAULT_GUILD_ID,
      summary: {
        action: "upsert",
        changed_count: -1,
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeWorkspaceGatewayEvent("workspace_unknown", {
      guild_id: DEFAULT_GUILD_ID,
    });

    expect(result).toBeNull();
  });

  it("fails closed for prototype-chain event types", () => {
    expect(isWorkspaceGatewayEventType("__proto__")).toBe(false);
  });
});
