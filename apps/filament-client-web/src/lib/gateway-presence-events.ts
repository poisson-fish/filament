import {
  type GuildId,
  guildIdFromInput,
  userIdFromInput,
} from "../domain/chat";

const MAX_PRESENCE_SYNC_USER_IDS = 1024;

type PresenceStatus = "online" | "offline";

export interface PresenceSyncPayload {
  guildId: GuildId;
  userIds: string[];
}

export interface PresenceUpdatePayload {
  guildId: GuildId;
  userId: string;
  status: PresenceStatus;
}

type PresenceGatewayEvent =
  | {
      type: "presence_sync";
      payload: PresenceSyncPayload;
    }
  | {
      type: "presence_update";
      payload: PresenceUpdatePayload;
    };

type PresenceGatewayEventType = PresenceGatewayEvent["type"];
type PresenceEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parsePresenceSyncPayload(payload: unknown): PresenceSyncPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string" || !Array.isArray(value.user_ids)) {
    return null;
  }
  if (value.user_ids.length > MAX_PRESENCE_SYNC_USER_IDS) {
    return null;
  }

  let guildId: GuildId;
  try {
    guildId = guildIdFromInput(value.guild_id);
  } catch {
    return null;
  }

  const seen = new Set<string>();
  const userIds: string[] = [];
  for (const entry of value.user_ids) {
    if (typeof entry !== "string") {
      return null;
    }

    let userId: string;
    try {
      userId = userIdFromInput(entry);
    } catch {
      return null;
    }

    if (seen.has(userId)) {
      continue;
    }
    seen.add(userId);
    userIds.push(userId);
  }

  return {
    guildId,
    userIds,
  };
}

function parsePresenceUpdatePayload(payload: unknown): PresenceUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    (value.status !== "online" && value.status !== "offline")
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
    status: value.status,
  };
}

const PRESENCE_EVENT_DECODERS: {
  [K in PresenceGatewayEventType]: PresenceEventDecoder<Extract<PresenceGatewayEvent, { type: K }>["payload"]>;
} = {
  presence_sync: parsePresenceSyncPayload,
  presence_update: parsePresenceUpdatePayload,
};

function isPresenceGatewayEventType(value: string): value is PresenceGatewayEventType {
  return value in PRESENCE_EVENT_DECODERS;
}

export function decodePresenceGatewayEvent(
  type: string,
  payload: unknown,
): PresenceGatewayEvent | null {
  if (!isPresenceGatewayEventType(type)) {
    return null;
  }

  if (type === "presence_sync") {
    const parsedPayload = PRESENCE_EVENT_DECODERS.presence_sync(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  const parsedPayload = PRESENCE_EVENT_DECODERS.presence_update(payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  };
}
