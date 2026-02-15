import {
  channelFromResponse,
  channelIdFromInput,
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  permissionFromInput,
  roleFromInput,
  userIdFromInput,
  type ChannelId,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type PermissionName,
  type RoleName,
} from "../domain/chat";
import type {
  ChannelCreatePayload,
  WorkspaceChannelOverrideUpdatePayload,
  WorkspaceIpBanSyncPayload,
  WorkspaceMemberAddPayload,
  WorkspaceMemberBanPayload,
  WorkspaceMemberRemovePayload,
  WorkspaceMemberUpdatePayload,
  WorkspaceUpdatePayload,
} from "./gateway-contracts";
import {
  decodeWorkspaceRoleGatewayEvent,
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
    }
  | {
      type: "workspace_member_add";
      payload: WorkspaceMemberAddPayload;
    }
  | {
      type: "workspace_member_update";
      payload: WorkspaceMemberUpdatePayload;
    }
  | {
      type: "workspace_member_remove";
      payload: WorkspaceMemberRemovePayload;
    }
  | {
      type: "workspace_member_ban";
      payload: WorkspaceMemberBanPayload;
    }
  | {
      type: "workspace_channel_override_update";
      payload: WorkspaceChannelOverrideUpdatePayload;
    }
  | {
      type: "workspace_ip_ban_sync";
      payload: WorkspaceIpBanSyncPayload;
    };

export type WorkspaceGatewayEvent = WorkspaceRoleGatewayEvent | WorkspaceNonRoleGatewayEvent;
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

function parseWorkspaceMemberAddPayload(payload: unknown): WorkspaceMemberAddPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role !== "string" ||
    typeof value.joined_at_unix !== "number" ||
    !Number.isSafeInteger(value.joined_at_unix) ||
    value.joined_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let role: RoleName;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    role = roleFromInput(value.role);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    role,
    joinedAtUnix: value.joined_at_unix,
  };
}

function parseWorkspaceMemberUpdatePayload(payload: unknown): WorkspaceMemberUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let role: RoleName | undefined;
  if (typeof updatedFieldsDto.role !== "undefined") {
    if (typeof updatedFieldsDto.role !== "string") {
      return null;
    }
    try {
      role = roleFromInput(updatedFieldsDto.role);
    } catch {
      return null;
    }
  }
  if (typeof role === "undefined") {
    return null;
  }

  return {
    guildId,
    userId,
    updatedFields: { role },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceMemberRemovePayload(payload: unknown): WorkspaceMemberRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    (value.reason !== "kick" && value.reason !== "ban" && value.reason !== "leave") ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    reason: value.reason,
    removedAtUnix: value.removed_at_unix,
  };
}

function parseWorkspaceMemberBanPayload(payload: unknown): WorkspaceMemberBanPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.banned_at_unix !== "number" ||
    !Number.isSafeInteger(value.banned_at_unix) ||
    value.banned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    bannedAtUnix: value.banned_at_unix,
  };
}

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

function parseWorkspaceIpBanSyncPayload(payload: unknown): WorkspaceIpBanSyncPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !value.summary ||
    typeof value.summary !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }
  const summaryDto = value.summary as Record<string, unknown>;
  if (
    (summaryDto.action !== "upsert" && summaryDto.action !== "remove") ||
    typeof summaryDto.changed_count !== "number" ||
    !Number.isSafeInteger(summaryDto.changed_count) ||
    summaryDto.changed_count < 0
  ) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  return {
    guildId,
    summary: {
      action: summaryDto.action,
      changedCount: summaryDto.changed_count,
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
  workspace_member_add: parseWorkspaceMemberAddPayload,
  workspace_member_update: parseWorkspaceMemberUpdatePayload,
  workspace_member_remove: parseWorkspaceMemberRemovePayload,
  workspace_member_ban: parseWorkspaceMemberBanPayload,
  workspace_channel_override_update: parseWorkspaceChannelOverrideUpdatePayload,
  workspace_ip_ban_sync: parseWorkspaceIpBanSyncPayload,
};

function isWorkspaceGatewayEventType(value: string): value is WorkspaceNonRoleGatewayEventType {
  return value in WORKSPACE_EVENT_DECODERS;
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

  if (!isWorkspaceGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceGatewayEvent(type, payload);
}
