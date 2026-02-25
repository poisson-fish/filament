import {
  channelIdFromInput,
  guildIdFromInput,
  permissionFromInput,
  roleFromInput,
  userIdFromInput,
  type ChannelId,
  type GuildId,
  type PermissionName,
  type RoleName,
} from "../domain/chat";
import type {
  WorkspaceChannelOverrideUpdatePayload,
  WorkspaceChannelPermissionOverrideUpdatePayload,
} from "./gateway-contracts";

type WorkspaceChannelOverrideGatewayEventType =
  | "workspace_channel_override_update"
  | "workspace_channel_role_override_update"
  | "workspace_channel_permission_override_update";

export type WorkspaceChannelOverrideGatewayEvent =
  | {
    type: "workspace_channel_override_update";
    payload: WorkspaceChannelOverrideUpdatePayload;
  }
  | {
    type: "workspace_channel_permission_override_update";
    payload: WorkspaceChannelPermissionOverrideUpdatePayload;
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

function parseWorkspaceChannelPermissionOverrideUpdatePayload(
  payload: unknown,
): WorkspaceChannelPermissionOverrideUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    (value.target_kind !== "role" && value.target_kind !== "member") ||
    typeof value.target_id !== "string" ||
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
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
  } catch {
    return null;
  }

  let targetId: string;
  if (value.target_kind === "role") {
    try {
      targetId = roleFromInput(value.target_id);
    } catch {
      return null;
    }
  } else {
    try {
      targetId = userIdFromInput(value.target_id);
    } catch {
      return null;
    }
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
    targetKind: value.target_kind,
    targetId,
    updatedFields: { allow, deny },
    updatedAtUnix: value.updated_at_unix,
  };
}

export function isWorkspaceChannelOverrideGatewayEventType(
  value: string,
): value is WorkspaceChannelOverrideGatewayEventType {
  return (
    value === "workspace_channel_override_update"
    || value === "workspace_channel_role_override_update"
    || value === "workspace_channel_permission_override_update"
  );
}

export function decodeWorkspaceChannelOverrideGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceChannelOverrideGatewayEvent | null {
  if (!isWorkspaceChannelOverrideGatewayEventType(type)) {
    return null;
  }

  const parsedRolePayload = parseWorkspaceChannelOverrideUpdatePayload(payload);
  if (parsedRolePayload) {
    return {
      type: "workspace_channel_override_update",
      payload: parsedRolePayload,
    };
  }

  const parsedPermissionPayload =
    parseWorkspaceChannelPermissionOverrideUpdatePayload(payload);
  if (parsedPermissionPayload) {
    return {
      type: "workspace_channel_permission_override_update",
      payload: parsedPermissionPayload,
    };
  }

  return null;
}
