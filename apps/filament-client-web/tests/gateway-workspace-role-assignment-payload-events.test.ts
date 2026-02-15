import { decodeWorkspaceRoleAssignmentGatewayEventPayload } from "../src/lib/gateway-workspace-role-assignment-payload-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const DEFAULT_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("decodeWorkspaceRoleAssignmentGatewayEventPayload", () => {
  it("decodes role assignment add payload through dedicated payload helper", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEventPayload("workspace_role_assignment_add", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      assigned_at_unix: 1710000001,
    });

    expect(result).toEqual({
      guildId: DEFAULT_GUILD_ID,
      userId: DEFAULT_USER_ID,
      roleId: DEFAULT_ROLE_ID,
      assignedAtUnix: 1710000001,
    });
  });

  it("fails closed for invalid role assignment remove payload", () => {
    const result = decodeWorkspaceRoleAssignmentGatewayEventPayload("workspace_role_assignment_remove", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      role_id: DEFAULT_ROLE_ID,
      removed_at_unix: 0,
    });

    expect(result).toBeNull();
  });
});
