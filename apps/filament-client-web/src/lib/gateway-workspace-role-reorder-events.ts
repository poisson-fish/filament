import { guildIdFromInput, workspaceRoleIdFromInput, type GuildId, type WorkspaceRoleId } from "../domain/chat";
import type { WorkspaceRoleReorderPayload } from "./gateway-contracts";

const MAX_WORKSPACE_ROLE_REORDER_IDS = 64;

export type WorkspaceRoleReorderGatewayEvent = {
  type: "workspace_role_reorder";
  payload: WorkspaceRoleReorderPayload;
};

type WorkspaceRoleReorderGatewayEventType = WorkspaceRoleReorderGatewayEvent["type"];

function parseWorkspaceRoleReorderPayload(payload: unknown): WorkspaceRoleReorderPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !Array.isArray(value.role_ids) ||
    value.role_ids.length > MAX_WORKSPACE_ROLE_REORDER_IDS ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const roleIds: WorkspaceRoleId[] = [];
  for (const entry of value.role_ids) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      roleIds.push(workspaceRoleIdFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    guildId,
    roleIds,
    updatedAtUnix: value.updated_at_unix,
  };
}

export function isWorkspaceRoleReorderGatewayEventType(
  value: string,
): value is WorkspaceRoleReorderGatewayEventType {
  return value === "workspace_role_reorder";
}

export function decodeWorkspaceRoleReorderGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleReorderGatewayEvent | null {
  if (!isWorkspaceRoleReorderGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleReorderPayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}