import {
  guildIdFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type PermissionName,
  type WorkspaceRoleId,
} from "../domain/chat";
import type { WorkspaceRoleCreatePayload, WorkspaceRoleRecordPayload } from "./gateway-contracts";

export type WorkspaceRoleCreateGatewayEvent = {
  type: "workspace_role_create";
  payload: WorkspaceRoleCreatePayload;
};

type WorkspaceRoleCreateGatewayEventType = WorkspaceRoleCreateGatewayEvent["type"];

function parseWorkspaceRolePayload(payload: unknown): WorkspaceRoleRecordPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.role_id !== "string" ||
    typeof value.name !== "string" ||
    typeof value.position !== "number" ||
    !Number.isSafeInteger(value.position) ||
    value.position < 1 ||
    typeof value.is_system !== "boolean" ||
    !Array.isArray(value.permissions)
  ) {
    return null;
  }

  let roleId: WorkspaceRoleId;
  try {
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  const permissions: PermissionName[] = [];
  for (const entry of value.permissions) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      permissions.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    roleId,
    name: value.name,
    position: value.position,
    isSystem: value.is_system,
    permissions,
  };
}

function parseWorkspaceRoleCreatePayload(payload: unknown): WorkspaceRoleCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string") {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const role = parseWorkspaceRolePayload(value.role);
  if (!role) {
    return null;
  }

  return { guildId, role };
}

export function isWorkspaceRoleCreateGatewayEventType(
  value: string,
): value is WorkspaceRoleCreateGatewayEventType {
  return value === "workspace_role_create";
}

export function decodeWorkspaceRoleCreateGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleCreateGatewayEvent | null {
  if (!isWorkspaceRoleCreateGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleCreatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}