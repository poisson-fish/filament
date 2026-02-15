import {
  guildIdFromInput,
  roleFromInput,
  userIdFromInput,
  type GuildId,
  type RoleName,
} from "../domain/chat";
import type {
  WorkspaceMemberAddPayload,
  WorkspaceMemberBanPayload,
  WorkspaceMemberRemovePayload,
  WorkspaceMemberUpdatePayload,
} from "./gateway-contracts";

export type WorkspaceMemberGatewayEvent =
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
    };

type WorkspaceMemberGatewayEventType = WorkspaceMemberGatewayEvent["type"];
type WorkspaceMemberEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

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

const WORKSPACE_MEMBER_EVENT_DECODERS: {
  [K in WorkspaceMemberGatewayEventType]: WorkspaceMemberEventDecoder<
    Extract<WorkspaceMemberGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_member_add: parseWorkspaceMemberAddPayload,
  workspace_member_update: parseWorkspaceMemberUpdatePayload,
  workspace_member_remove: parseWorkspaceMemberRemovePayload,
  workspace_member_ban: parseWorkspaceMemberBanPayload,
};

export function isWorkspaceMemberGatewayEventType(
  value: string,
): value is WorkspaceMemberGatewayEventType {
  return value in WORKSPACE_MEMBER_EVENT_DECODERS;
}

function decodeKnownWorkspaceMemberGatewayEvent<K extends WorkspaceMemberGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<WorkspaceMemberGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_MEMBER_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceMemberGatewayEvent, { type: K }>;
}

export function decodeWorkspaceMemberGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberGatewayEvent | null {
  if (!isWorkspaceMemberGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceMemberGatewayEvent(type, payload);
}