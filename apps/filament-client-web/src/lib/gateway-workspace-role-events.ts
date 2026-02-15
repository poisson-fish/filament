import {
  decodeWorkspaceRoleCreateGatewayEvent,
  isWorkspaceRoleCreateGatewayEventType,
  type WorkspaceRoleCreateGatewayEvent,
} from "./gateway-workspace-role-create-events";
import {
  isWorkspaceRoleAssignmentGatewayEventType,
  type WorkspaceRoleAssignmentGatewayEvent,
} from "./gateway-workspace-role-assignment-events";
import { decodeWorkspaceRoleAssignmentAddGatewayEvent } from "./gateway-workspace-role-assignment-add-events";
import { decodeWorkspaceRoleAssignmentRemoveGatewayEvent } from "./gateway-workspace-role-assignment-remove-events";
import {
  decodeWorkspaceRoleReorderGatewayEvent,
  isWorkspaceRoleReorderGatewayEventType,
  type WorkspaceRoleReorderGatewayEvent,
} from "./gateway-workspace-role-reorder-events";
import {
  decodeWorkspaceRoleDeleteGatewayEvent,
  isWorkspaceRoleDeleteGatewayEventType,
  type WorkspaceRoleDeleteGatewayEvent,
} from "./gateway-workspace-role-delete-events";
import {
  decodeWorkspaceRoleUpdateGatewayEvent,
  isWorkspaceRoleUpdateGatewayEventType,
  type WorkspaceRoleUpdateGatewayEvent,
} from "./gateway-workspace-role-update-events";

export type WorkspaceRoleGatewayEvent =
  | WorkspaceRoleCreateGatewayEvent
  | WorkspaceRoleUpdateGatewayEvent
  | WorkspaceRoleDeleteGatewayEvent
  | WorkspaceRoleReorderGatewayEvent
  | WorkspaceRoleAssignmentGatewayEvent;
type WorkspaceRoleGatewayEventType = WorkspaceRoleGatewayEvent["type"];
type WorkspaceRoleEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

const WORKSPACE_ROLE_EVENT_DECODERS: {
  [K in WorkspaceRoleGatewayEventType]: WorkspaceRoleEventDecoder<
    Extract<WorkspaceRoleGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_role_create: (payload) =>
    decodeWorkspaceRoleCreateGatewayEvent("workspace_role_create", payload)?.payload ?? null,
  workspace_role_update: (payload) =>
    decodeWorkspaceRoleUpdateGatewayEvent("workspace_role_update", payload)?.payload ?? null,
  workspace_role_delete: (payload) =>
    decodeWorkspaceRoleDeleteGatewayEvent("workspace_role_delete", payload)?.payload ?? null,
  workspace_role_reorder: (payload) =>
    decodeWorkspaceRoleReorderGatewayEvent("workspace_role_reorder", payload)?.payload ?? null,
  workspace_role_assignment_add: (payload) =>
    decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_add", payload)
      ?.payload ?? null,
  workspace_role_assignment_remove: (payload) =>
    decodeWorkspaceRoleAssignmentRemoveGatewayEvent("workspace_role_assignment_remove", payload)
      ?.payload ?? null,
};

export function isWorkspaceRoleGatewayEventType(
  value: string,
): value is WorkspaceRoleGatewayEventType {
  return (
    isWorkspaceRoleCreateGatewayEventType(value) ||
    isWorkspaceRoleUpdateGatewayEventType(value) ||
    isWorkspaceRoleDeleteGatewayEventType(value) ||
    isWorkspaceRoleReorderGatewayEventType(value) ||
    isWorkspaceRoleAssignmentGatewayEventType(value) ||
    value in WORKSPACE_ROLE_EVENT_DECODERS
  );
}

function decodeKnownWorkspaceRoleGatewayEvent<K extends WorkspaceRoleGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<WorkspaceRoleGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_ROLE_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceRoleGatewayEvent, { type: K }>;
}

export function decodeWorkspaceRoleGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleGatewayEvent | null {
  if (!isWorkspaceRoleGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceRoleGatewayEvent(type, payload);
}