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
type WorkspaceRoleAssignmentGatewayEventDecoder = (
  type: string,
  payload: unknown,
) => WorkspaceRoleAssignmentGatewayEvent | null;

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_TYPE_GUARDS: ReadonlyArray<(value: string) => boolean> = [
  isWorkspaceRoleAssignmentAddGatewayEventType,
  isWorkspaceRoleAssignmentRemoveGatewayEventType,
];

const WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODER_REGISTRY: ReadonlyArray<
  WorkspaceRoleAssignmentGatewayEventDecoder
> = [
  decodeWorkspaceRoleAssignmentAddGatewayEvent,
  decodeWorkspaceRoleAssignmentRemoveGatewayEvent,
];

export function isWorkspaceRoleAssignmentGatewayEventType(
  value: string,
): value is WorkspaceRoleAssignmentGatewayEventType {
  return WORKSPACE_ROLE_ASSIGNMENT_EVENT_TYPE_GUARDS.some((guard) => guard(value));
}

export function decodeWorkspaceRoleAssignmentGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceRoleAssignmentGatewayEvent | null {
  if (!isWorkspaceRoleAssignmentGatewayEventType(type)) {
    return null;
  }

  for (const decoder of WORKSPACE_ROLE_ASSIGNMENT_EVENT_DECODER_REGISTRY) {
    const decodedEvent = decoder(type, payload);
    if (decodedEvent) {
      return decodedEvent;
    }
  }

  return null;
}