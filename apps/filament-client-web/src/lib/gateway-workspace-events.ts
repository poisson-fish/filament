import {
  channelFromResponse,
  guildIdFromInput,
  type GuildId,
} from "../domain/chat";
import type {
  ChannelCreatePayload,
} from "./gateway-contracts";
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
  {
    type: "channel_create";
    payload: ChannelCreatePayload;
  };

export type WorkspaceGatewayEvent =
  | WorkspaceRoleGatewayEvent
  | WorkspaceMemberGatewayEvent
  | WorkspaceIpBanGatewayEvent
  | WorkspaceChannelOverrideGatewayEvent
  | WorkspaceUpdateGatewayEvent
  | WorkspaceNonRoleGatewayEvent;
export type WorkspaceGatewayEventType = WorkspaceGatewayEvent["type"];
type WorkspaceNonRoleGatewayEventType = WorkspaceNonRoleGatewayEvent["type"];
type WorkspaceEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseChannelCreatePayload(payload: unknown): ChannelCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string") {
    return null;
  }

  let guildId: GuildId;
  let channel: ChannelCreatePayload["channel"];
  try {
    guildId = guildIdFromInput(value.guild_id);
    channel = channelFromResponse(value.channel);
  } catch {
    return null;
  }

  return {
    guildId,
    channel,
  };
}

const WORKSPACE_EVENT_DECODERS: {
  [K in WorkspaceNonRoleGatewayEventType]: WorkspaceEventDecoder<
    Extract<WorkspaceNonRoleGatewayEvent, { type: K }>["payload"]
  >;
} = {
  channel_create: parseChannelCreatePayload,
};

function isWorkspaceNonRoleGatewayEventType(
  value: string,
): value is WorkspaceNonRoleGatewayEventType {
  return value in WORKSPACE_EVENT_DECODERS;
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

function decodeKnownWorkspaceGatewayEvent<K extends WorkspaceNonRoleGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<WorkspaceNonRoleGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceNonRoleGatewayEvent, { type: K }>;
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

  return decodeKnownWorkspaceGatewayEvent(type, payload);
}
