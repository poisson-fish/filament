import {
  guildIdFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  type GuildId,
  type WorkspaceRoleId,
} from "../domain/chat";
import type {
  WorkspaceRoleAssignmentAddPayload,
  WorkspaceRoleAssignmentRemovePayload,
} from "./gateway-contracts";

export type WorkspaceRoleAssignmentGatewayEvent =
  | {
      type: "workspace_role_assignment_add";
      payload: WorkspaceRoleAssignmentAddPayload;
    }
  | {
      type: "workspace_role_assignment_remove";
      payload: WorkspaceRoleAssignmentRemovePayload;
    };

type WorkspaceRoleAssignmentGatewayEventType = WorkspaceRoleAssignmentGatewayEvent["type"];
type WorkspaceRoleAssignmentEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

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

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS: {
  [K in WorkspaceRoleAssignmentGatewayEventType]: WorkspaceRoleAssignmentEventDecoder<
    Extract<WorkspaceRoleAssignmentGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_role_assignment_add: parseWorkspaceRoleAssignmentAddPayload,
  workspace_role_assignment_remove: parseWorkspaceRoleAssignmentRemovePayload,
};

export function isWorkspaceRoleAssignmentGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentGatewayEventType {
  return value in WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS;
}

function decodeKnownWorkspaceRoleAssignmentGatewayEvent<
  K extends WorkspaceRoleAssignmentGatewayEventType,
>(
  type: K,
  payload: unknown,
): Extract<WorkspaceRoleAssignmentGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceRoleAssignmentGatewayEvent, { type: K }>;
}

export function decodeWorkspaceRoleAssignmentGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleAssignmentGatewayEvent | null {
  if (!isWorkspaceRoleAssignmentGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceRoleAssignmentGatewayEvent(type, payload);
}