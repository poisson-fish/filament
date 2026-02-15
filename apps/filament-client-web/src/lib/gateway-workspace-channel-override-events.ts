import {
  channelIdFromInput,
  guildIdFromInput,
  permissionFromInput,
  roleFromInput,
  type ChannelId,
  type GuildId,
  type PermissionName,
  type RoleName,
} from "../domain/chat";
import type { WorkspaceChannelOverrideUpdatePayload } from "./gateway-contracts";

export type WorkspaceChannelOverrideGatewayEvent = {
  type: "workspace_channel_override_update";
  payload: WorkspaceChannelOverrideUpdatePayload;
};

function parseWorkspaceChannelOverrideUpdatePayload(
  payload: unknown,
): WorkspaceChannelOverrideUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.role !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  let role: RoleName;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    role = roleFromInput(value.role);
  } catch {
    return null;
  }

  const updatedFields = value.updated_fields as Record<string, unknown>;
  if (!Array.isArray(updatedFields.allow) || !Array.isArray(updatedFields.deny)) {
    return null;
  }
  const allow: PermissionName[] = [];
  for (const entry of updatedFields.allow) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      allow.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }
  const deny: PermissionName[] = [];
  for (const entry of updatedFields.deny) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      deny.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    guildId,
    channelId,
    role,
    updatedFields: { allow, deny },
    updatedAtUnix: value.updated_at_unix,
  };
}

export function isWorkspaceChannelOverrideGatewayEventType(
  value: string,
): value is "workspace_channel_override_update" {
  return value === "workspace_channel_override_update";
}

export function decodeWorkspaceChannelOverrideGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceChannelOverrideGatewayEvent | null {
  if (!isWorkspaceChannelOverrideGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceChannelOverrideUpdatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}
