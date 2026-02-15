import {
  guildIdFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type WorkspaceRoleId,
} from "../domain/chat";
import type { WorkspaceRoleAssignmentRemovePayload } from "./gateway-contracts";

export type WorkspaceRoleAssignmentRemoveGatewayEvent = {
  type: "workspace_role_assignment_remove";
  payload: WorkspaceRoleAssignmentRemovePayload;
};

type WorkspaceRoleAssignmentRemoveGatewayEventType =
  WorkspaceRoleAssignmentRemoveGatewayEvent["type"];

function parseWorkspaceRoleAssignmentRemovePayload(
  payload: unknown,
): WorkspaceRoleAssignmentRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
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
    removedAtUnix: value.removed_at_unix,
  };
}

export function isWorkspaceRoleAssignmentRemoveGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentRemoveGatewayEventType {
  return value === "workspace_role_assignment_remove";
}

export function decodeWorkspaceRoleAssignmentRemoveGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleAssignmentRemoveGatewayEvent | null {
  if (!isWorkspaceRoleAssignmentRemoveGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleAssignmentRemovePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}
