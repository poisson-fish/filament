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
type WorkspaceGatewayDomain =
  | "role"
  | "member"
  | "ipBan"
  | "channelOverride"
  | "workspaceUpdate"
  | "channel";

function isWorkspaceNonRoleGatewayEventType(
  value: string,
): value is WorkspaceNonRoleGatewayEventType {
  return isWorkspaceChannelGatewayEventType(value);
}

const WORKSPACE_EVENT_TYPE_GUARDS: {
  [K in WorkspaceGatewayDomain]: (value: string) => boolean;
} = {
  role: isWorkspaceRoleGatewayEventType,
  member: isWorkspaceMemberGatewayEventType,
  ipBan: isWorkspaceIpBanGatewayEventType,
  channelOverride: isWorkspaceChannelOverrideGatewayEventType,
  workspaceUpdate: isWorkspaceUpdateGatewayEventType,
  channel: isWorkspaceNonRoleGatewayEventType,
};

const WORKSPACE_EVENT_DECODER_REGISTRY: {
  [K in WorkspaceGatewayDomain]: WorkspaceGatewayEventDecoder;
} = {
  role: decodeWorkspaceRoleGatewayEvent,
  member: decodeWorkspaceMemberGatewayEvent,
  ipBan: decodeWorkspaceIpBanGatewayEvent,
  channelOverride: decodeWorkspaceChannelOverrideGatewayEvent,
  workspaceUpdate: decodeWorkspaceUpdateGatewayEvent,
  channel: decodeWorkspaceChannelGatewayEvent,
};

const WORKSPACE_EVENT_DOMAINS: readonly WorkspaceGatewayDomain[] = [
  "role",
  "member",
  "ipBan",
  "channelOverride",
  "workspaceUpdate",
  "channel",
];

const hasOwn = Object.prototype.hasOwnProperty;

export function isWorkspaceGatewayEventType(
  value: string,
): value is WorkspaceGatewayEventType {
  return WORKSPACE_EVENT_DOMAINS.some((domain) => {
    if (!hasOwn.call(WORKSPACE_EVENT_TYPE_GUARDS, domain)) {
      return false;
    }
    return WORKSPACE_EVENT_TYPE_GUARDS[domain](value);
  });
}

export function decodeWorkspaceGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceGatewayEvent | null {
  if (!isWorkspaceGatewayEventType(type)) {
    return null;
  }

  for (const domain of WORKSPACE_EVENT_DOMAINS) {
    if (!hasOwn.call(WORKSPACE_EVENT_DECODER_REGISTRY, domain)) {
      continue;
    }
    const decoder = WORKSPACE_EVENT_DECODER_REGISTRY[domain];
    const decodedEvent = decoder(type, payload);
    if (decodedEvent) {
      return decodedEvent;
    }
  }

  return null;
}
