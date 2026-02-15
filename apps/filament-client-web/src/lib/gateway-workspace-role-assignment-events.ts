import {
  decodeWorkspaceRoleAssignmentAddGatewayEvent,
  isWorkspaceRoleAssignmentAddGatewayEventType,
  type WorkspaceRoleAssignmentAddGatewayEvent,
} from "./gateway-workspace-role-assignment-add-events";
import {
  decodeWorkspaceRoleAssignmentRemoveGatewayEvent,
  isWorkspaceRoleAssignmentRemoveGatewayEventType,
  type WorkspaceRoleAssignmentRemoveGatewayEvent,
} from "./gateway-workspace-role-assignment-remove-events";

export type WorkspaceRoleAssignmentGatewayEvent =
  | WorkspaceRoleAssignmentAddGatewayEvent
  | WorkspaceRoleAssignmentRemoveGatewayEvent;

type WorkspaceRoleAssignmentGatewayEventType = WorkspaceRoleAssignmentGatewayEvent["type"];
type WorkspaceRoleAssignmentEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS: {
  [K in WorkspaceRoleAssignmentGatewayEventType]: WorkspaceRoleAssignmentEventDecoder<
    Extract<WorkspaceRoleAssignmentGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_role_assignment_add: (payload) =>
    decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_add", payload)
      ?.payload ?? null,
  workspace_role_assignment_remove: (payload) =>
    decodeWorkspaceRoleAssignmentRemoveGatewayEvent("workspace_role_assignment_remove", payload)
      ?.payload ?? null,
};

export function isWorkspaceRoleAssignmentGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentGatewayEventType {
  return (
    isWorkspaceRoleAssignmentAddGatewayEventType(value) ||
    isWorkspaceRoleAssignmentRemoveGatewayEventType(value) ||
    value in WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS
  );
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