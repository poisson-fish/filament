import {
  decodeWorkspaceChannelOverrideGatewayEvent,
  isWorkspaceChannelOverrideGatewayEventType,
} from "../src/lib/gateway-workspace-channel-override-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("decodeWorkspaceChannelOverrideGatewayEvent", () => {
  it("exposes strict workspace channel override event type guard", () => {
    expect(
      isWorkspaceChannelOverrideGatewayEventType("workspace_channel_role_override_update"),
    ).toBe(true);
    expect(
      isWorkspaceChannelOverrideGatewayEventType("workspace_channel_permission_override_update"),
    ).toBe(true);
    expect(isWorkspaceChannelOverrideGatewayEventType("workspace_update")).toBe(false);
  });

  it("decodes valid workspace_channel_role_override_update payload", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_role_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        role: "member",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
    );

    expect(result).toEqual({
      type: "workspace_channel_role_override_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        role: "member",
        updatedFields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("decodes valid workspace_channel_permission_override_update payload", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_permission_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        target_kind: "member",
        target_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
    );

    expect(result).toEqual({
      type: "workspace_channel_permission_override_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        targetKind: "member",
        targetId: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
        updatedFields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed for invalid permission payload entries", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_role_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        role: "member",
        updated_fields: {
          allow: ["create_message"],
          deny: ["not_a_permission"],
        },
        updated_at_unix: 1710000001,
      },
    );

    expect(result).toBeNull();
  });

  it("fails closed for role override payload missing deny permission set", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_role_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        role: "member",
        updated_fields: {
          allow: ["create_message"],
        },
        updated_at_unix: 1710000001,
      },
    );

    expect(result).toBeNull();
  });

  it("fails closed for permission override payload with invalid target kind", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_permission_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        target_kind: "invalid",
        target_id: "member",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
    );

    expect(result).toBeNull();
  });

  it("fails closed when role event carries permission payload shape", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_role_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        target_kind: "member",
        target_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
    );
    expect(result).toBeNull();
  });

  it("fails closed when permission event carries role payload shape", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent(
      "workspace_channel_permission_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        role: "member",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
    );
    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeWorkspaceChannelOverrideGatewayEvent("workspace_unknown", {
      guild_id: DEFAULT_GUILD_ID,
    });

    expect(result).toBeNull();
  });
});
