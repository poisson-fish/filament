import { guildIdFromInput, workspaceRoleIdFromInput, type GuildId, type WorkspaceRoleId } from "../domain/chat";
import type { WorkspaceRoleDeletePayload } from "./gateway-contracts";

export type WorkspaceRoleDeleteGatewayEvent = {
  type: "workspace_role_delete";
  payload: WorkspaceRoleDeletePayload;
};

type WorkspaceRoleDeleteGatewayEventType = WorkspaceRoleDeleteGatewayEvent["type"];

function parseWorkspaceRoleDeletePayload(payload: unknown): WorkspaceRoleDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    roleId,
    deletedAtUnix: value.deleted_at_unix,
  };
}

export function isWorkspaceRoleDeleteGatewayEventType(
  value: string,
): value is WorkspaceRoleDeleteGatewayEventType {
  return value === "workspace_role_delete";
}

export function decodeWorkspaceRoleDeleteGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleDeleteGatewayEvent | null {
  if (!isWorkspaceRoleDeleteGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleDeletePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}