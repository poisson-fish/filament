import {
  decodeWorkspaceRoleAssignmentGatewayEvent,
  type WorkspaceRoleAssignmentGatewayEvent,
} from "./gateway-workspace-role-assignment-events";

type WorkspaceRoleAssignmentGatewayEventType = WorkspaceRoleAssignmentGatewayEvent["type"];

export function decodeWorkspaceRoleAssignmentGatewayEventPayload(
  type: "workspace_role_assignment_add",
  payload: unknown,
): Extract<WorkspaceRoleAssignmentGatewayEvent, { type: "workspace_role_assignment_add" }>["payload"] | null;
export function decodeWorkspaceRoleAssignmentGatewayEventPayload(
  type: "workspace_role_assignment_remove",
  payload: unknown,
): Extract<WorkspaceRoleAssignmentGatewayEvent, { type: "workspace_role_assignment_remove" }>["payload"] | null;
export function decodeWorkspaceRoleAssignmentGatewayEventPayload(
  type: WorkspaceRoleAssignmentGatewayEventType,
  payload: unknown,
): WorkspaceRoleAssignmentGatewayEvent["payload"] | null {
  const decodedEvent = decodeWorkspaceRoleAssignmentGatewayEvent(type, payload);
  if (!decodedEvent || decodedEvent.type !== type) {
    return null;
  }

  return decodedEvent.payload;
}