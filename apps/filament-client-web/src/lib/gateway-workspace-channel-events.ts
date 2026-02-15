import {
  channelFromResponse,
  guildIdFromInput,
  type GuildId,
} from "../domain/chat";
import type { ChannelCreatePayload } from "./gateway-contracts";

export type WorkspaceChannelGatewayEvent = {
  type: "channel_create";
  payload: ChannelCreatePayload;
};

export function isWorkspaceChannelGatewayEventType(value: string): value is "channel_create" {
  return value === "channel_create";
}

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

export function decodeWorkspaceChannelGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceChannelGatewayEvent | null {
  if (!isWorkspaceChannelGatewayEventType(type)) {
    return null;
  }

  const parsedPayload = parseChannelCreatePayload(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}