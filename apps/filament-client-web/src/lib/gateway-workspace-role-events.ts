import {
  guildIdFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type PermissionName,
  type WorkspaceRoleId,
} from "../domain/chat";
import type {
  WorkspaceRoleCreatePayload,
  WorkspaceRoleDeletePayload,
  WorkspaceRoleRecordPayload,
  WorkspaceRoleUpdatePayload,
} from "./gateway-contracts";
import {
  decodeWorkspaceRoleAssignmentGatewayEvent,
  isWorkspaceRoleAssignmentGatewayEventType,
  type WorkspaceRoleAssignmentGatewayEvent,
} from "./gateway-workspace-role-assignment-events";
import {
  decodeWorkspaceRoleReorderGatewayEvent,
  isWorkspaceRoleReorderGatewayEventType,
  type WorkspaceRoleReorderGatewayEvent,
} from "./gateway-workspace-role-reorder-events";

export type WorkspaceRoleGatewayEvent =
  | {
      type: "workspace_role_create";
      payload: WorkspaceRoleCreatePayload;
    }
  | {
      type: "workspace_role_update";
      payload: WorkspaceRoleUpdatePayload;
    }
  | {
      type: "workspace_role_delete";
      payload: WorkspaceRoleDeletePayload;
    }
  | WorkspaceRoleReorderGatewayEvent
  | WorkspaceRoleAssignmentGatewayEvent;

type WorkspaceRoleCoreGatewayEvent = Exclude<
  WorkspaceRoleGatewayEvent,
  WorkspaceRoleAssignmentGatewayEvent | WorkspaceRoleReorderGatewayEvent
>;
type WorkspaceRoleGatewayEventType = WorkspaceRoleGatewayEvent["type"];
type WorkspaceRoleCoreGatewayEventType = WorkspaceRoleCoreGatewayEvent["type"];
type WorkspaceRoleEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

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

const WORKSPACE_ROLE_EVENT_DECODERS: {
  [K in WorkspaceRoleCoreGatewayEventType]: WorkspaceRoleEventDecoder<
    Extract<WorkspaceRoleCoreGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_role_create: parseWorkspaceRoleCreatePayload,
  workspace_role_update: parseWorkspaceRoleUpdatePayload,
  workspace_role_delete: parseWorkspaceRoleDeletePayload,
};

export function isWorkspaceRoleGatewayEventType(
  value: string,
): value is WorkspaceRoleGatewayEventType {
  return (
    value in WORKSPACE_ROLE_EVENT_DECODERS ||
    isWorkspaceRoleReorderGatewayEventType(value) ||
    isWorkspaceRoleAssignmentGatewayEventType(value)
  );
}

function isWorkspaceRoleCoreGatewayEventType(value: string): value is WorkspaceRoleCoreGatewayEventType {
  return value in WORKSPACE_ROLE_EVENT_DECODERS;
}

function decodeKnownWorkspaceRoleGatewayEvent<K extends WorkspaceRoleCoreGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<WorkspaceRoleCoreGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_ROLE_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceRoleCoreGatewayEvent, { type: K }>;
}

export function decodeWorkspaceRoleGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleGatewayEvent | null {
  const reorderEvent = decodeWorkspaceRoleReorderGatewayEvent(type, payload);
  if (reorderEvent) {
    return reorderEvent;
  }

  const assignmentEvent = decodeWorkspaceRoleAssignmentGatewayEvent(type, payload);
  if (assignmentEvent) {
    return assignmentEvent;
  }

  if (!isWorkspaceRoleGatewayEventType(type)) {
    return null;
  }

  if (!isWorkspaceRoleCoreGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceRoleGatewayEvent(type, payload);
}