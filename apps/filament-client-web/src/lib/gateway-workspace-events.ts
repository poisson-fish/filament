import {
  channelFromResponse,
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  type GuildId,
  type GuildName,
  type GuildVisibility,
} from "../domain/chat";
import type {
  ChannelCreatePayload,
  WorkspaceUpdatePayload,
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

type WorkspaceNonRoleGatewayEvent =
  | {
      type: "channel_create";
      payload: ChannelCreatePayload;
    }
  | {
      type: "workspace_update";
      payload: WorkspaceUpdatePayload;
    };

export type WorkspaceGatewayEvent =
  | WorkspaceRoleGatewayEvent
  | WorkspaceMemberGatewayEvent
  | WorkspaceIpBanGatewayEvent
  | WorkspaceChannelOverrideGatewayEvent
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

function parseWorkspaceUpdatePayload(payload: unknown): WorkspaceUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let name: GuildName | undefined;
  let visibility: GuildVisibility | undefined;
  if (typeof updatedFieldsDto.name !== "undefined") {
    if (typeof updatedFieldsDto.name !== "string") {
      return null;
    }
    try {
      name = guildNameFromInput(updatedFieldsDto.name);
    } catch {
      return null;
    }
  }
  if (typeof updatedFieldsDto.visibility !== "undefined") {
    if (typeof updatedFieldsDto.visibility !== "string") {
      return null;
    }
    try {
      visibility = guildVisibilityFromInput(updatedFieldsDto.visibility);
    } catch {
      return null;
    }
  }
  if (typeof name === "undefined" && typeof visibility === "undefined") {
    return null;
  }

  return {
    guildId,
    updatedFields: {
      name,
      visibility,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

const WORKSPACE_EVENT_DECODERS: {
  [K in WorkspaceNonRoleGatewayEventType]: WorkspaceEventDecoder<
    Extract<WorkspaceNonRoleGatewayEvent, { type: K }>["payload"]
  >;
} = {
  channel_create: parseChannelCreatePayload,
  workspace_update: parseWorkspaceUpdatePayload,
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

  if (!isWorkspaceNonRoleGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceGatewayEvent(type, payload);
}
