import {
  decodeWorkspaceRoleAssignmentGatewayEvent,
  isWorkspaceRoleAssignmentGatewayEventType,
} from "../src/lib/gateway-workspace-role-assignment-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleAssignmentGatewayEvent", () => {
  it("decodes valid workspace_role_assignment_add payload via aggregate assignment decoder", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEvent("workspace_role_assignment_add", {
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

  it("fails closed for invalid workspace_role_assignment_remove payload", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEvent("workspace_role_assignment_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      removed_at_unix: 0,
    });

    expect(result).toBeNull();
  });

  it("decodes valid workspace_role_assignment_remove payload via aggregate assignment decoder", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEvent("workspace_role_assignment_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      removed_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "workspace_role_assignment_remove",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userId: DEFAULT_USER_ID,
        roleId: DEFAULT_ROLE_ID,
        removedAtUnix: 1710000001,
      },
    });
  });

  it("returns null for unknown role assignment event type", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEvent("workspace_role_assignment_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleAssignmentGatewayEventType", () => {
  it("classifies only role assignment event types", () => {
    expect(isWorkspaceRoleAssignmentGatewayEventType("workspace_role_assignment_add")).toBe(true);
    expect(isWorkspaceRoleAssignmentGatewayEventType("workspace_role_assignment_remove")).toBe(true);
    expect(isWorkspaceRoleAssignmentGatewayEventType("workspace_role_create")).toBe(false);
  });
});