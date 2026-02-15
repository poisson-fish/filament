import {
  channelIdFromInput,
  guildIdFromInput,
  userIdFromInput,
  type ChannelId,
  type GuildId,
} from "../domain/chat";
import type {
  VoiceStreamKind,
  VoiceStreamPublishPayload,
  VoiceStreamUnpublishPayload,
} from "./gateway-contracts";

export type VoiceStreamGatewayEvent =
  | {
      type: "voice_stream_publish";
      payload: VoiceStreamPublishPayload;
    }
  | {
      type: "voice_stream_unpublish";
      payload: VoiceStreamUnpublishPayload;
    };

type VoiceStreamGatewayEventType = VoiceStreamGatewayEvent["type"];
type VoiceStreamEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseVoiceStreamKind(value: unknown): VoiceStreamKind | null {
  if (value === "microphone" || value === "camera" || value === "screen_share") {
    return value;
  }
  return null;
}

function parseVoiceStreamPublishPayload(payload: unknown): VoiceStreamPublishPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.identity !== "string" ||
    typeof value.published_at_unix !== "number" ||
    !Number.isSafeInteger(value.published_at_unix) ||
    value.published_at_unix < 1
  ) {
    return null;
  }
  const stream = parseVoiceStreamKind(value.stream);
  if (!stream) {
    return null;
  }
  let guildId: GuildId;
  let channelId: ChannelId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }
  return {
    guildId,
    channelId,
    userId,
    identity: value.identity,
    stream,
    publishedAtUnix: value.published_at_unix,
  };
}

function parseVoiceStreamUnpublishPayload(payload: unknown): VoiceStreamUnpublishPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.identity !== "string" ||
    typeof value.unpublished_at_unix !== "number" ||
    !Number.isSafeInteger(value.unpublished_at_unix) ||
    value.unpublished_at_unix < 1
  ) {
    return null;
  }
  const stream = parseVoiceStreamKind(value.stream);
  if (!stream) {
    return null;
  }
  let guildId: GuildId;
  let channelId: ChannelId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }
  return {
    guildId,
    channelId,
    userId,
    identity: value.identity,
    stream,
    unpublishedAtUnix: value.unpublished_at_unix,
  };
}

const VOICE_STREAM_EVENT_DECODERS: {
  [K in VoiceStreamGatewayEventType]: VoiceStreamEventDecoder<
    Extract<VoiceStreamGatewayEvent, { type: K }>["payload"]
  >;
} = {
  voice_stream_publish: parseVoiceStreamPublishPayload,
  voice_stream_unpublish: parseVoiceStreamUnpublishPayload,
};

export function isVoiceStreamGatewayEventType(
  value: string,
): value is VoiceStreamGatewayEventType {
  return value in VOICE_STREAM_EVENT_DECODERS;
}

function decodeKnownVoiceStreamGatewayEvent<K extends VoiceStreamGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<VoiceStreamGatewayEvent, { type: K }> | null {
  const parsedPayload = VOICE_STREAM_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<VoiceStreamGatewayEvent, { type: K }>;
}

export function decodeVoiceStreamGatewayEvent(
  type: string,
  payload: unknown,
): VoiceStreamGatewayEvent | null {
  if (!isVoiceStreamGatewayEventType(type)) {
    return null;
  }

  return decodeKnownVoiceStreamGatewayEvent(type, payload);
}