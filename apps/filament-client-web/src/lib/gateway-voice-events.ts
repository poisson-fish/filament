import {
  channelIdFromInput,
  guildIdFromInput,
  userIdFromInput,
  type ChannelId,
  type GuildId,
} from "../domain/chat";
import type {
  VoiceParticipantJoinPayload,
  VoiceParticipantLeavePayload,
  VoiceParticipantPayload,
  VoiceParticipantSyncPayload,
  VoiceParticipantUpdatePayload,
} from "./gateway-contracts";
import {
  decodeVoiceStreamGatewayEvent,
  isVoiceStreamGatewayEventType,
  type VoiceStreamGatewayEvent,
} from "./gateway-voice-stream-events";

const MAX_VOICE_PARTICIPANT_SYNC_SIZE = 512;

type VoiceGatewayEvent =
  | {
      type: "voice_participant_sync";
      payload: VoiceParticipantSyncPayload;
    }
  | {
      type: "voice_participant_join";
      payload: VoiceParticipantJoinPayload;
    }
  | {
      type: "voice_participant_leave";
      payload: VoiceParticipantLeavePayload;
    }
  | {
      type: "voice_participant_update";
      payload: VoiceParticipantUpdatePayload;
    }
  | VoiceStreamGatewayEvent;

type VoiceGatewayEventType = VoiceGatewayEvent["type"];
type VoiceParticipantGatewayEventType =
  | "voice_participant_sync"
  | "voice_participant_join"
  | "voice_participant_leave"
  | "voice_participant_update";
type VoiceEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseVoiceParticipant(payload: unknown): VoiceParticipantPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.user_id !== "string" ||
    typeof value.identity !== "string" ||
    value.identity.length < 1 ||
    value.identity.length > 512 ||
    typeof value.joined_at_unix !== "number" ||
    !Number.isSafeInteger(value.joined_at_unix) ||
    value.joined_at_unix < 1 ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1 ||
    typeof value.is_muted !== "boolean" ||
    typeof value.is_deafened !== "boolean" ||
    typeof value.is_speaking !== "boolean" ||
    typeof value.is_video_enabled !== "boolean" ||
    typeof value.is_screen_share_enabled !== "boolean"
  ) {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    userId,
    identity: value.identity,
    joinedAtUnix: value.joined_at_unix,
    updatedAtUnix: value.updated_at_unix,
    isMuted: value.is_muted,
    isDeafened: value.is_deafened,
    isSpeaking: value.is_speaking,
    isVideoEnabled: value.is_video_enabled,
    isScreenShareEnabled: value.is_screen_share_enabled,
  };
}

function parseVoiceParticipantSyncPayload(payload: unknown): VoiceParticipantSyncPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    !Array.isArray(value.participants) ||
    value.participants.length > MAX_VOICE_PARTICIPANT_SYNC_SIZE ||
    typeof value.synced_at_unix !== "number" ||
    !Number.isSafeInteger(value.synced_at_unix) ||
    value.synced_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let channelId: ChannelId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
  } catch {
    return null;
  }

  const participants: VoiceParticipantPayload[] = [];
  const seen = new Set<string>();
  for (const entry of value.participants) {
    const participant = parseVoiceParticipant(entry);
    if (!participant) {
      return null;
    }
    if (seen.has(participant.identity)) {
      continue;
    }
    seen.add(participant.identity);
    participants.push(participant);
  }

  return {
    guildId,
    channelId,
    participants,
    syncedAtUnix: value.synced_at_unix,
  };
}

function parseVoiceParticipantJoinPayload(payload: unknown): VoiceParticipantJoinPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (typeof value.guild_id !== "string" || typeof value.channel_id !== "string") {
    return null;
  }
  let guildId: GuildId;
  let channelId: ChannelId;
  try {
    guildId = guildIdFromInput(value.guild_id);
    channelId = channelIdFromInput(value.channel_id);
  } catch {
    return null;
  }
  const participant = parseVoiceParticipant(value.participant);
  if (!participant) {
    return null;
  }
  return { guildId, channelId, participant };
}

function parseVoiceParticipantLeavePayload(payload: unknown): VoiceParticipantLeavePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.identity !== "string" ||
    typeof value.left_at_unix !== "number" ||
    !Number.isSafeInteger(value.left_at_unix) ||
    value.left_at_unix < 1
  ) {
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
    leftAtUnix: value.left_at_unix,
  };
}

function parseVoiceParticipantUpdatePayload(payload: unknown): VoiceParticipantUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.channel_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.identity !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
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
  const fields = value.updated_fields as Record<string, unknown>;
  let isMuted: boolean | undefined;
  let isDeafened: boolean | undefined;
  let isSpeaking: boolean | undefined;
  let isVideoEnabled: boolean | undefined;
  let isScreenShareEnabled: boolean | undefined;
  if (typeof fields.is_muted !== "undefined") {
    if (typeof fields.is_muted !== "boolean") {
      return null;
    }
    isMuted = fields.is_muted;
  }
  if (typeof fields.is_deafened !== "undefined") {
    if (typeof fields.is_deafened !== "boolean") {
      return null;
    }
    isDeafened = fields.is_deafened;
  }
  if (typeof fields.is_speaking !== "undefined") {
    if (typeof fields.is_speaking !== "boolean") {
      return null;
    }
    isSpeaking = fields.is_speaking;
  }
  if (typeof fields.is_video_enabled !== "undefined") {
    if (typeof fields.is_video_enabled !== "boolean") {
      return null;
    }
    isVideoEnabled = fields.is_video_enabled;
  }
  if (typeof fields.is_screen_share_enabled !== "undefined") {
    if (typeof fields.is_screen_share_enabled !== "boolean") {
      return null;
    }
    isScreenShareEnabled = fields.is_screen_share_enabled;
  }
  if (
    typeof isMuted === "undefined" &&
    typeof isDeafened === "undefined" &&
    typeof isSpeaking === "undefined" &&
    typeof isVideoEnabled === "undefined" &&
    typeof isScreenShareEnabled === "undefined"
  ) {
    return null;
  }

  return {
    guildId,
    channelId,
    userId,
    identity: value.identity,
    updatedFields: {
      isMuted,
      isDeafened,
      isSpeaking,
      isVideoEnabled,
      isScreenShareEnabled,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

const VOICE_EVENT_DECODERS: {
  [K in VoiceParticipantGatewayEventType]: VoiceEventDecoder<
    Extract<VoiceGatewayEvent, { type: K }>["payload"]
  >;
} = {
  voice_participant_sync: parseVoiceParticipantSyncPayload,
  voice_participant_join: parseVoiceParticipantJoinPayload,
  voice_participant_leave: parseVoiceParticipantLeavePayload,
  voice_participant_update: parseVoiceParticipantUpdatePayload,
};

export function isVoiceGatewayEventType(value: string): value is VoiceGatewayEventType {
  return value in VOICE_EVENT_DECODERS || isVoiceStreamGatewayEventType(value);
}

export function decodeVoiceGatewayEvent(
  type: string,
  payload: unknown,
): VoiceGatewayEvent | null {
  if (!isVoiceGatewayEventType(type)) {
    return null;
  }

  if (isVoiceStreamGatewayEventType(type)) {
    return decodeVoiceStreamGatewayEvent(type, payload);
  }

  if (type === "voice_participant_sync") {
    const parsedPayload = VOICE_EVENT_DECODERS.voice_participant_sync(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "voice_participant_join") {
    const parsedPayload = VOICE_EVENT_DECODERS.voice_participant_join(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "voice_participant_leave") {
    const parsedPayload = VOICE_EVENT_DECODERS.voice_participant_leave(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "voice_participant_update") {
    const parsedPayload = VOICE_EVENT_DECODERS.voice_participant_update(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  return null;
}