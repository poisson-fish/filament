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

export function isWorkspaceGatewayEventType(
  value: string,
): value is WorkspaceGatewayEventType {
  return (
    isWorkspaceRoleGatewayEventType(value) ||
    isWorkspaceMemberGatewayEventType(value) ||
    isWorkspaceIpBanGatewayEventType(value) ||
    isWorkspaceChannelOverrideGatewayEventType(value) ||
    isWorkspaceUpdateGatewayEventType(value) ||
    isWorkspaceNonRoleGatewayEventType(value)
  );
}

export function decodeWorkspaceGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceGatewayEvent | null {
  const roleEvent = decodeWorkspaceRoleGatewayEvent(type, payload);
  if (roleEvent) {
    return roleEvent;
  }

  const memberEvent = decodeWorkspaceMemberGatewayEvent(type, payload);
  if (memberEvent) {
    return memberEvent;
  }

  const ipBanEvent = decodeWorkspaceIpBanGatewayEvent(type, payload);
  if (ipBanEvent) {
    return ipBanEvent;
  }

  const channelOverrideEvent = decodeWorkspaceChannelOverrideGatewayEvent(type, payload);
  if (channelOverrideEvent) {
    return channelOverrideEvent;
  }

  const workspaceUpdateEvent = decodeWorkspaceUpdateGatewayEvent(type, payload);
  if (workspaceUpdateEvent) {
    return workspaceUpdateEvent;
  }

  if (!isWorkspaceNonRoleGatewayEventType(type)) {
    return null;
  }

  return decodeWorkspaceChannelGatewayEvent(type, payload);
}
