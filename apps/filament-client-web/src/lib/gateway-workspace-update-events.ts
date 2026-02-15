import {
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  type GuildId,
  type GuildName,
  type GuildVisibility,
} from "../domain/chat";
import type { WorkspaceUpdatePayload } from "./gateway-contracts";

export type WorkspaceUpdateGatewayEvent = {
  type: "workspace_update";
  payload: WorkspaceUpdatePayload;
};

export function isWorkspaceUpdateGatewayEventType(value: string): value is "workspace_update" {
  return value === "workspace_update";
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

export function decodeWorkspaceUpdateGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceUpdateGatewayEvent | null {
  if (!isWorkspaceUpdateGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceUpdatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}