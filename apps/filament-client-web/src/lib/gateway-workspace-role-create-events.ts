import {
  guildIdFromInput,
  permissionFromInput,
  roleColorHexFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type PermissionName,
  type RoleColorHex,
  type WorkspaceRoleId,
} from "../domain/chat";
import type { WorkspaceRoleCreatePayload, WorkspaceRoleRecordPayload } from "./gateway-contracts";

export type WorkspaceRoleCreateGatewayEvent = {
  type: "workspace_role_create";
  payload: WorkspaceRoleCreatePayload;
};

type WorkspaceRoleCreateGatewayEventType = WorkspaceRoleCreateGatewayEvent["type"];
const hasOwn = Object.prototype.hasOwnProperty;

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

  let colorHex: RoleColorHex | null | undefined;
  if (hasOwn.call(value, "color_hex")) {
    if (value.color_hex === null) {
      colorHex = null;
    } else if (typeof value.color_hex === "string") {
      try {
        colorHex = roleColorHexFromInput(value.color_hex);
      } catch {
        return null;
      }
    } else {
      return null;
    }
  }

  const role: WorkspaceRoleRecordPayload = {
    roleId,
    name: value.name,
    position: value.position,
    isSystem: value.is_system,
    permissions,
  };
  if (typeof colorHex !== "undefined") {
    role.colorHex = colorHex;
  }

  return role;
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
