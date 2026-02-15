import {
  guildIdFromInput,
  roleFromInput,
  userIdFromInput,
  type GuildId,
  type RoleName,
} from "../domain/chat";
import type { WorkspaceMemberAddPayload } from "./gateway-contracts";

export type WorkspaceMemberAddGatewayEvent = {
  type: "workspace_member_add";
  payload: WorkspaceMemberAddPayload;
};

type WorkspaceMemberAddGatewayEventType = WorkspaceMemberAddGatewayEvent["type"];

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

export function isWorkspaceMemberAddGatewayEventType(
  value: string,
): value is WorkspaceMemberAddGatewayEventType {
  return value === "workspace_member_add";
}

export function decodeWorkspaceMemberAddGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberAddGatewayEvent | null {
  if (!isWorkspaceMemberAddGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceMemberAddPayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}