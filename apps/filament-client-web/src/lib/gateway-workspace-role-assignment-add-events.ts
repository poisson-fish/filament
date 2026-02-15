import {
  guildIdFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type WorkspaceRoleId,
} from "../domain/chat";
import type { WorkspaceRoleAssignmentAddPayload } from "./gateway-contracts";

export type WorkspaceRoleAssignmentAddGatewayEvent = {
  type: "workspace_role_assignment_add";
  payload: WorkspaceRoleAssignmentAddPayload;
};

type WorkspaceRoleAssignmentAddGatewayEventType = WorkspaceRoleAssignmentAddGatewayEvent["type"];

function parseWorkspaceRoleAssignmentAddPayload(
  payload: unknown,
): WorkspaceRoleAssignmentAddPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.assigned_at_unix !== "number" ||
    !Number.isSafeInteger(value.assigned_at_unix) ||
    value.assigned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    roleId,
    assignedAtUnix: value.assigned_at_unix,
  };
}

export function isWorkspaceRoleAssignmentAddGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentAddGatewayEventType {
  return value === "workspace_role_assignment_add";
}

export function decodeWorkspaceRoleAssignmentAddGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleAssignmentAddGatewayEvent | null {
  if (!isWorkspaceRoleAssignmentAddGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleAssignmentAddPayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}
