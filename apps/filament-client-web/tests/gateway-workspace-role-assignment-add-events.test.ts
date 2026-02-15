import {
  decodeWorkspaceRoleAssignmentAddGatewayEvent,
  isWorkspaceRoleAssignmentAddGatewayEventType,
} from "../src/lib/gateway-workspace-role-assignment-add-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleAssignmentAddGatewayEvent", () => {
  it("decodes valid workspace_role_assignment_add payload", () => {
    const result = decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_add", {
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

  it("fails closed for invalid workspace_role_assignment_add payload", () => {
    const result = decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      assigned_at_unix: 0,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown assignment add event type", () => {
    const result = decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      assigned_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});

describe("isWorkspaceRoleAssignmentAddGatewayEventType", () => {
  it("classifies only assignment add event types", () => {
    expect(isWorkspaceRoleAssignmentAddGatewayEventType("workspace_role_assignment_add")).toBe(
      true,
    );
    expect(isWorkspaceRoleAssignmentAddGatewayEventType("workspace_role_assignment_remove")).toBe(
      false,
    );
  });
});
