import { guildIdFromInput, userIdFromInput, type GuildId } from "../domain/chat";
import type { WorkspaceMemberRemovePayload } from "./gateway-contracts";

export type WorkspaceMemberRemoveGatewayEvent = {
  type: "workspace_member_remove";
  payload: WorkspaceMemberRemovePayload;
};

type WorkspaceMemberRemoveGatewayEventType = WorkspaceMemberRemoveGatewayEvent["type"];

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

export function isWorkspaceMemberRemoveGatewayEventType(
  value: string,
): value is WorkspaceMemberRemoveGatewayEventType {
  return value === "workspace_member_remove";
}

export function decodeWorkspaceMemberRemoveGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberRemoveGatewayEvent | null {
  if (!isWorkspaceMemberRemoveGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceMemberRemovePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}