import {
  guildIdFromInput,
  userIdFromInput,
  type GuildId,
} from "../domain/chat";
import type { WorkspaceMemberBanPayload } from "./gateway-contracts";

export type WorkspaceMemberBanGatewayEvent = {
  type: "workspace_member_ban";
  payload: WorkspaceMemberBanPayload;
};

type WorkspaceMemberBanGatewayEventType = WorkspaceMemberBanGatewayEvent["type"];

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

export function isWorkspaceMemberBanGatewayEventType(
  value: string,
): value is WorkspaceMemberBanGatewayEventType {
  return value === "workspace_member_ban";
}

export function decodeWorkspaceMemberBanGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberBanGatewayEvent | null {
  if (!isWorkspaceMemberBanGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceMemberBanPayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}