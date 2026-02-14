import {
  channelFromResponse,
  channelIdFromInput,
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  permissionFromInput,
  roleFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  type ChannelId,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type PermissionName,
  type RoleName,
  type WorkspaceRoleId,
} from "../domain/chat";
import type {
  ChannelCreatePayload,
  WorkspaceChannelOverrideUpdatePayload,
  WorkspaceIpBanSyncPayload,
  WorkspaceMemberAddPayload,
  WorkspaceMemberBanPayload,
  WorkspaceMemberRemovePayload,
  WorkspaceMemberUpdatePayload,
  WorkspaceRoleAssignmentAddPayload,
  WorkspaceRoleAssignmentRemovePayload,
  WorkspaceRoleCreatePayload,
  WorkspaceRoleDeletePayload,
  WorkspaceRoleRecordPayload,
  WorkspaceRoleReorderPayload,
  WorkspaceRoleUpdatePayload,
  WorkspaceUpdatePayload,
} from "./gateway";

const MAX_WORKSPACE_ROLE_REORDER_IDS = 64;

type WorkspaceGatewayEvent =
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
      type: "workspace_role_create";
      payload: WorkspaceRoleCreatePayload;
    }
  | {
      type: "workspace_role_update";
      payload: WorkspaceRoleUpdatePayload;
    }
  | {
      type: "workspace_role_delete";
      payload: WorkspaceRoleDeletePayload;
    }
  | {
      type: "workspace_role_reorder";
      payload: WorkspaceRoleReorderPayload;
    }
  | {
      type: "workspace_role_assignment_add";
      payload: WorkspaceRoleAssignmentAddPayload;
    }
  | {
      type: "workspace_role_assignment_remove";
      payload: WorkspaceRoleAssignmentRemovePayload;
    }
  | {
      type: "workspace_channel_override_update";
      payload: WorkspaceChannelOverrideUpdatePayload;
    }
  | {
      type: "workspace_ip_ban_sync";
      payload: WorkspaceIpBanSyncPayload;
    };

type WorkspaceGatewayEventType = WorkspaceGatewayEvent["type"];
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

function parseWorkspaceRolePayload(payload: unknown): WorkspaceRoleRecordPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.role_id !== "string" ||
    typeof value.name !== "string" ||
    typeof value.position !== "number" ||
    !Number.isSafeInteger(value.position) ||
    value.position < 1 ||
    typeof value.is_system !== "boolean" ||
    !Array.isArray(value.permissions)
  ) {
    return null;
  }

  let roleId: WorkspaceRoleId;
  try {
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  const permissions: PermissionName[] = [];
  for (const entry of value.permissions) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      permissions.push(permissionFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    roleId,
    name: value.name,
    position: value.position,
    isSystem: value.is_system,
    permissions,
  };
}

function parseWorkspaceRoleCreatePayload(payload: unknown): WorkspaceRoleCreatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string") {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }
  const role = parseWorkspaceRolePayload(value.role);
  if (!role) {
    return null;
  }

  return { guildId, role };
}

function parseWorkspaceRoleUpdatePayload(payload: unknown): WorkspaceRoleUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let name: string | undefined;
  let permissions: PermissionName[] | undefined;
  if (typeof updatedFieldsDto.name !== "undefined") {
    if (typeof updatedFieldsDto.name !== "string") {
      return null;
    }
    name = updatedFieldsDto.name;
  }
  if (typeof updatedFieldsDto.permissions !== "undefined") {
    if (!Array.isArray(updatedFieldsDto.permissions)) {
      return null;
    }
    permissions = [];
    for (const entry of updatedFieldsDto.permissions) {
      if (typeof entry !== "string") {
        return null;
      }
      try {
        permissions.push(permissionFromInput(entry));
      } catch {
        return null;
      }
    }
  }
  if (typeof name === "undefined" && typeof permissions === "undefined") {
    return null;
  }

  return {
    guildId,
    roleId,
    updatedFields: {
      name,
      permissions,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceRoleDeletePayload(payload: unknown): WorkspaceRoleDeletePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.deleted_at_unix !== "number" ||
    !Number.isSafeInteger(value.deleted_at_unix) ||
    value.deleted_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    roleId,
    deletedAtUnix: value.deleted_at_unix,
  };
}

function parseWorkspaceRoleReorderPayload(payload: unknown): WorkspaceRoleReorderPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    !Array.isArray(value.role_ids) ||
    value.role_ids.length > MAX_WORKSPACE_ROLE_REORDER_IDS ||
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

  const roleIds: WorkspaceRoleId[] = [];
  for (const entry of value.role_ids) {
    if (typeof entry !== "string") {
      return null;
    }
    try {
      roleIds.push(workspaceRoleIdFromInput(entry));
    } catch {
      return null;
    }
  }

  return {
    guildId,
    roleIds,
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseWorkspaceRoleAssignmentAddPayload(
  payload: unknown,
): WorkspaceRoleAssignmentAddPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.assigned_at_unix !== "number" ||
    !Number.isSafeInteger(value.assigned_at_unix) ||
    value.assigned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    roleId,
    assignedAtUnix: value.assigned_at_unix,
  };
}

function parseWorkspaceRoleAssignmentRemovePayload(
  payload: unknown,
): WorkspaceRoleAssignmentRemovePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.role_id !== "string" ||
    typeof value.removed_at_unix !== "number" ||
    !Number.isSafeInteger(value.removed_at_unix) ||
    value.removed_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  let roleId: WorkspaceRoleId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
    roleId = workspaceRoleIdFromInput(value.role_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    roleId,
    removedAtUnix: value.removed_at_unix,
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
  [K in WorkspaceGatewayEventType]: WorkspaceEventDecoder<
    Extract<WorkspaceGatewayEvent, { type: K }>["payload"]
  >;
} = {
  channel_create: parseChannelCreatePayload,
  workspace_update: parseWorkspaceUpdatePayload,
  workspace_member_add: parseWorkspaceMemberAddPayload,
  workspace_member_update: parseWorkspaceMemberUpdatePayload,
  workspace_member_remove: parseWorkspaceMemberRemovePayload,
  workspace_member_ban: parseWorkspaceMemberBanPayload,
  workspace_role_create: parseWorkspaceRoleCreatePayload,
  workspace_role_update: parseWorkspaceRoleUpdatePayload,
  workspace_role_delete: parseWorkspaceRoleDeletePayload,
  workspace_role_reorder: parseWorkspaceRoleReorderPayload,
  workspace_role_assignment_add: parseWorkspaceRoleAssignmentAddPayload,
  workspace_role_assignment_remove: parseWorkspaceRoleAssignmentRemovePayload,
  workspace_channel_override_update: parseWorkspaceChannelOverrideUpdatePayload,
  workspace_ip_ban_sync: parseWorkspaceIpBanSyncPayload,
};

function isWorkspaceGatewayEventType(value: string): value is WorkspaceGatewayEventType {
  return value in WORKSPACE_EVENT_DECODERS;
}

export function decodeWorkspaceGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceGatewayEvent | null {
  if (!isWorkspaceGatewayEventType(type)) {
    return null;
  }

  switch (type) {
    case "channel_create": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.channel_create(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_update": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_update(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_member_add": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_member_add(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_member_update": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_member_update(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_member_remove": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_member_remove(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_member_ban": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_member_ban(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_create": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_create(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_update": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_update(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_delete": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_delete(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_reorder": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_reorder(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_assignment_add": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_assignment_add(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_role_assignment_remove": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_role_assignment_remove(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_channel_override_update": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_channel_override_update(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
    case "workspace_ip_ban_sync": {
      const parsedPayload = WORKSPACE_EVENT_DECODERS.workspace_ip_ban_sync(payload);
      if (!parsedPayload) {
        return null;
      }
      return { type, payload: parsedPayload };
    }
  }
}
