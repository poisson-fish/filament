import {
  decodeWorkspaceChannelGatewayEvent,
  isWorkspaceChannelGatewayEventType,
  type WorkspaceChannelGatewayEvent,
} from "./gateway-workspace-channel-events";
import {
  decodeWorkspaceChannelOverrideGatewayEvent,
  isWorkspaceChannelOverrideGatewayEventType,
  type WorkspaceChannelOverrideGatewayEvent,
} from "./gateway-workspace-channel-override-events";
import {
  decodeWorkspaceIpBanGatewayEvent,
  isWorkspaceIpBanGatewayEventType,
  type WorkspaceIpBanGatewayEvent,
} from "./gateway-workspace-ip-ban-events";
import {
  decodeWorkspaceMemberGatewayEvent,
  isWorkspaceMemberGatewayEventType,
  type WorkspaceMemberGatewayEvent,
} from "./gateway-workspace-member-events";
import {
  decodeWorkspaceRoleGatewayEvent,
  isWorkspaceRoleGatewayEventType,
  type WorkspaceRoleGatewayEvent,
} from "./gateway-workspace-role-events";
import {
  decodeWorkspaceUpdateGatewayEvent,
  isWorkspaceUpdateGatewayEventType,
  type WorkspaceUpdateGatewayEvent,
} from "./gateway-workspace-update-events";

type WorkspaceNonRoleGatewayEvent =
  WorkspaceChannelGatewayEvent;

type WorkspaceGatewayEventDecoder = (
  type: string,
  payload: unknown,
) => WorkspaceGatewayEvent | null;

export type WorkspaceGatewayEvent =
  | WorkspaceRoleGatewayEvent
  | WorkspaceMemberGatewayEvent
  | WorkspaceIpBanGatewayEvent
  | WorkspaceChannelOverrideGatewayEvent
  | WorkspaceUpdateGatewayEvent
  | WorkspaceNonRoleGatewayEvent;
export type WorkspaceGatewayEventType = WorkspaceGatewayEvent["type"];
type WorkspaceNonRoleGatewayEventType = WorkspaceNonRoleGatewayEvent["type"];

function isWorkspaceNonRoleGatewayEventType(
  value: string,
): value is WorkspaceNonRoleGatewayEventType {
  return isWorkspaceChannelGatewayEventType(value);
}

const WORKSPACE_EVENT_TYPE_GUARDS: ReadonlyArray<(value: string) => boolean> = [
  isWorkspaceRoleGatewayEventType,
  isWorkspaceMemberGatewayEventType,
  isWorkspaceIpBanGatewayEventType,
  isWorkspaceChannelOverrideGatewayEventType,
  isWorkspaceUpdateGatewayEventType,
  isWorkspaceNonRoleGatewayEventType,
];

const WORKSPACE_EVENT_DECODER_REGISTRY: ReadonlyArray<WorkspaceGatewayEventDecoder> = [
  decodeWorkspaceRoleGatewayEvent,
  decodeWorkspaceMemberGatewayEvent,
  decodeWorkspaceIpBanGatewayEvent,
  decodeWorkspaceChannelOverrideGatewayEvent,
  decodeWorkspaceUpdateGatewayEvent,
  decodeWorkspaceChannelGatewayEvent,
];

export function isWorkspaceGatewayEventType(
  value: string,
): value is WorkspaceGatewayEventType {
  return WORKSPACE_EVENT_TYPE_GUARDS.some((guard) => guard(value));
}

export function decodeWorkspaceGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceGatewayEvent | null {
  if (!isWorkspaceGatewayEventType(type)) {
    return null;
  }

  for (const decoder of WORKSPACE_EVENT_DECODER_REGISTRY) {
    const decodedEvent = decoder(type, payload);
    if (decodedEvent) {
      return decodedEvent;
    }
  }

  return null;
}
