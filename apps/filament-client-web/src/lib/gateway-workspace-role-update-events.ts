import {
  guildIdFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type PermissionName,
  type WorkspaceRoleId,
} from "../domain/chat";
import type { WorkspaceRoleUpdatePayload } from "./gateway-contracts";

export type WorkspaceRoleUpdateGatewayEvent = {
  type: "workspace_role_update";
  payload: WorkspaceRoleUpdatePayload;
};

type WorkspaceRoleUpdateGatewayEventType = WorkspaceRoleUpdateGatewayEvent["type"];

function parseWorkspaceRoleUpdatePayload(payload: unknown): WorkspaceRoleUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
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

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let name: string | undefined;
  let permissions: PermissionName[] | undefined;
  if (typeof updatedFieldsDto.name !== "undefined") {
    if (typeof updatedFieldsDto.name !== "string") {
      return null;
    }
    name = updatedFieldsDto.name;
  }
  if (typeof updatedFieldsDto.permissions !== "undefined") {
    if (!Array.isArray(updatedFieldsDto.permissions)) {
      return null;
    }
    permissions = [];
    for (const entry of updatedFieldsDto.permissions) {
      if (typeof entry !== "string") {
        return null;
      }
      try {
        permissions.push(permissionFromInput(entry));
      } catch {
        return null;
      }
    }
  }
  if (typeof name === "undefined" && typeof permissions === "undefined") {
    return null;
  }

  return {
    guildId,
    roleId,
    updatedFields: {
      name,
      permissions,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

export function isWorkspaceRoleUpdateGatewayEventType(
  value: string,
): value is WorkspaceRoleUpdateGatewayEventType {
  return value === "workspace_role_update";
}

export function decodeWorkspaceRoleUpdateGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleUpdateGatewayEvent | null {
  if (!isWorkspaceRoleUpdateGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceRoleUpdatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}