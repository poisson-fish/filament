import { guildIdFromInput, type GuildId } from "../domain/chat";
import type { WorkspaceIpBanSyncPayload } from "./gateway-contracts";

export type WorkspaceIpBanGatewayEvent = {
  type: "workspace_ip_ban_sync";
  payload: WorkspaceIpBanSyncPayload;
};

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

export function isWorkspaceIpBanGatewayEventType(value: string): value is "workspace_ip_ban_sync" {
  return value === "workspace_ip_ban_sync";
}

export function decodeWorkspaceIpBanGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceIpBanGatewayEvent | null {
  if (!isWorkspaceIpBanGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseWorkspaceIpBanSyncPayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}