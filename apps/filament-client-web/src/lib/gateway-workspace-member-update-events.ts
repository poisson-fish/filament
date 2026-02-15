import { guildIdFromInput, roleFromInput, userIdFromInput, type GuildId, type RoleName } from "../domain/chat";
import type { WorkspaceMemberUpdatePayload } from "./gateway-contracts";

export type WorkspaceMemberUpdateGatewayEvent = {
  type: "workspace_member_update";
  payload: WorkspaceMemberUpdatePayload;
};

type WorkspaceMemberUpdateGatewayEventType = WorkspaceMemberUpdateGatewayEvent["type"];

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

export function isWorkspaceMemberUpdateGatewayEventType(
  value: string,
): value is WorkspaceMemberUpdateGatewayEventType {
  return value === "workspace_member_update";
}

export function decodeWorkspaceMemberUpdateGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberUpdateGatewayEvent | null {
  if (!isWorkspaceMemberUpdateGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceMemberUpdatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}
