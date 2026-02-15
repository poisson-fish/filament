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
type WorkspaceRoleAssignmentGatewayEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_TYPE_GUARDS: {
  [K in WorkspaceRoleAssignmentGatewayEventType]: (value: string) => value is K;
} = {
  workspace_role_assignment_add: isWorkspaceRoleAssignmentAddGatewayEventType,
  workspace_role_assignment_remove: isWorkspaceRoleAssignmentRemoveGatewayEventType,
};

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODERS: {
  [K in WorkspaceRoleAssignmentGatewayEventType]: WorkspaceRoleAssignmentGatewayEventDecoder<
    Extract<WorkspaceRoleAssignmentGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_role_assignment_add: (payload) =>
    decodeWorkspaceRoleAssignmentAddGatewayEvent("workspace_role_assignment_add", payload)?.payload ?? null,
  workspace_role_assignment_remove: (payload) =>
    decodeWorkspaceRoleAssignmentRemoveGatewayEvent("workspace_role_assignment_remove", payload)?.payload ??
    null,
};

const hasOwn = Object.prototype.hasOwnProperty;

export function isWorkspaceRoleAssignmentGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentGatewayEventType {
  return (
    hasOwn.call(WORKSPACE_ROLE_ASSIGNMENT_EVENT_TYPE_GUARDS, value) &&
    WORKSPACE_ROLE_ASSIGNMENT_EVENT_TYPE_GUARDS[
      value as WorkspaceRoleAssignmentGatewayEventType
    ](value)
  );
}

function decodeKnownWorkspaceRoleAssignmentGatewayEvent<K extends WorkspaceRoleAssignmentGatewayEventType>(
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