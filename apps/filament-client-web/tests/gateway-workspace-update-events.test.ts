import {
  decodeWorkspaceUpdateGatewayEvent,
  isWorkspaceUpdateGatewayEventType,
} from "../src/lib/gateway-workspace-update-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("decodeWorkspaceUpdateGatewayEvent", () => {
  it("exposes strict workspace update event type guard", () => {
    expect(isWorkspaceUpdateGatewayEventType("workspace_update")).toBe(true);
    expect(isWorkspaceUpdateGatewayEventType("channel_create")).toBe(false);
  });

  it("decodes valid workspace_update payload", () => {
    const result = decodeWorkspaceUpdateGatewayEvent("workspace_update", {
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

  it("fails closed when updated_fields has no valid deltas", () => {
    const result = decodeWorkspaceUpdateGatewayEvent("workspace_update", {
      guild_id: DEFAULT_GUILD_ID,
      updated_fields: {},
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeWorkspaceUpdateGatewayEvent("workspace_unknown", {
      guild_id: DEFAULT_GUILD_ID,
    });

    expect(result).toBeNull();
  });
});