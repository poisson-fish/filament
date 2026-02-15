import {
  decodeWorkspaceRoleCreateGatewayEvent,
  isWorkspaceRoleCreateGatewayEventType,
  type WorkspaceRoleCreateGatewayEvent,
} from "./gateway-workspace-role-create-events";
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

export function isWorkspaceRoleGatewayEventType(
  value: string,
): value is WorkspaceRoleGatewayEventType {
  return (
    isWorkspaceRoleCreateGatewayEventType(value) ||
    isWorkspaceRoleUpdateGatewayEventType(value) ||
    isWorkspaceRoleDeleteGatewayEventType(value) ||
    isWorkspaceRoleReorderGatewayEventType(value) ||
    isWorkspaceRoleAssignmentGatewayEventType(value)
  );
}

export function decodeWorkspaceRoleGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleGatewayEvent | null {
  const createEvent = decodeWorkspaceRoleCreateGatewayEvent(type, payload);
  if (createEvent) {
    return createEvent;
  }

  const deleteEvent = decodeWorkspaceRoleDeleteGatewayEvent(type, payload);
  if (deleteEvent) {
    return deleteEvent;
  }

  const reorderEvent = decodeWorkspaceRoleReorderGatewayEvent(type, payload);
  if (reorderEvent) {
    return reorderEvent;
  }

  const assignmentEvent = decodeWorkspaceRoleAssignmentGatewayEvent(type, payload);
  if (assignmentEvent) {
    return assignmentEvent;
  }

  const updateEvent = decodeWorkspaceRoleUpdateGatewayEvent(type, payload);
  if (updateEvent) {
    return updateEvent;
  }

  return null;
}