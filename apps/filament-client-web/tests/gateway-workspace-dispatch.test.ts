import {
  dispatchWorkspaceGatewayEvent,
} from "../src/lib/gateway-workspace-dispatch";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("dispatchWorkspaceGatewayEvent", () => {
  it("dispatches decoded workspace events to matching handlers", () => {
    const onChannelCreate = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "channel_create",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel: {
          channel_id: DEFAULT_CHANNEL_ID,
          name: "general",
          kind: "text",
        },
      },
      { onChannelCreate },
    );

    expect(handled).toBe(true);
    expect(onChannelCreate).toHaveBeenCalledTimes(1);
    expect(onChannelCreate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channel: {
        channelId: DEFAULT_CHANNEL_ID,
        name: "general",
        kind: "text",
      },
    });
  });

  it("fails closed for known workspace event types with invalid payloads", () => {
    const onWorkspaceMemberAdd = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "workspace_member_add",
      {
        guild_id: DEFAULT_GUILD_ID,
        user_id: DEFAULT_USER_ID,
        role: "",
        joined_at_unix: 1710000001,
      },
      { onWorkspaceMemberAdd },
    );

    expect(handled).toBe(true);
    expect(onWorkspaceMemberAdd).not.toHaveBeenCalled();
  });

  it("dispatches role override updates to legacy override handler", () => {
    const onWorkspaceChannelOverrideUpdate = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "workspace_channel_override_update",
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
      { onWorkspaceChannelOverrideUpdate },
    );

    expect(handled).toBe(true);
    expect(onWorkspaceChannelOverrideUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceChannelOverrideUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      role: "member",
      updatedFields: {
        allow: ["create_message"],
        deny: ["ban_member"],
      },
      updatedAtUnix: 1710000001,
    });
  });

  it("normalizes legacy permission payload shape to permission override handler", () => {
    const onWorkspaceChannelOverrideUpdate = vi.fn();
    const onWorkspaceChannelPermissionOverrideUpdate = vi.fn();
    const onWorkspaceIpBanSync = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "workspace_channel_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        target_kind: "member",
        target_id: DEFAULT_USER_ID,
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
      {
        onWorkspaceChannelOverrideUpdate,
        onWorkspaceChannelPermissionOverrideUpdate,
        onWorkspaceIpBanSync,
      },
    );

    expect(handled).toBe(true);
    expect(onWorkspaceChannelOverrideUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceChannelPermissionOverrideUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceChannelPermissionOverrideUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      targetKind: "member",
      targetId: DEFAULT_USER_ID,
      updatedFields: {
        allow: ["create_message"],
        deny: ["ban_member"],
      },
      updatedAtUnix: 1710000001,
    });
    expect(onWorkspaceIpBanSync).not.toHaveBeenCalled();
  });

  it("dispatches explicit permission override event type to permission override handler", () => {
    const onWorkspaceChannelPermissionOverrideUpdate = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "workspace_channel_permission_override_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        target_kind: "role",
        target_id: "member",
        updated_fields: {
          allow: ["create_message"],
          deny: ["ban_member"],
        },
        updated_at_unix: 1710000001,
      },
      { onWorkspaceChannelPermissionOverrideUpdate },
    );

    expect(handled).toBe(true);
    expect(onWorkspaceChannelPermissionOverrideUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceChannelPermissionOverrideUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      targetKind: "role",
      targetId: "member",
      updatedFields: {
        allow: ["create_message"],
        deny: ["ban_member"],
      },
      updatedAtUnix: 1710000001,
    });
  });

  it("returns false for non-workspace event types", () => {
    const onWorkspaceIpBanSync = vi.fn();

    const handled = dispatchWorkspaceGatewayEvent(
      "profile_update",
      {},
      { onWorkspaceIpBanSync },
    );

    expect(handled).toBe(false);
    expect(onWorkspaceIpBanSync).not.toHaveBeenCalled();
  });
});
