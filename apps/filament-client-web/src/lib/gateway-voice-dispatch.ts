import type {
  VoiceParticipantJoinPayload,
  VoiceParticipantLeavePayload,
  VoiceParticipantSyncPayload,
  VoiceParticipantUpdatePayload,
  VoiceStreamPublishPayload,
  VoiceStreamUnpublishPayload,
} from "./gateway";
import {
  decodeVoiceGatewayEvent,
} from "./gateway-voice-events";

export interface VoiceGatewayDispatchHandlers {
  onVoiceParticipantSync?: (payload: VoiceParticipantSyncPayload) => void;
  onVoiceParticipantJoin?: (payload: VoiceParticipantJoinPayload) => void;
  onVoiceParticipantLeave?: (payload: VoiceParticipantLeavePayload) => void;
  onVoiceParticipantUpdate?: (payload: VoiceParticipantUpdatePayload) => void;
  onVoiceStreamPublish?: (payload: VoiceStreamPublishPayload) => void;
  onVoiceStreamUnpublish?: (payload: VoiceStreamUnpublishPayload) => void;
}

const VOICE_GATEWAY_EVENT_TYPES = new Set<string>([
  "voice_participant_sync",
  "voice_participant_join",
  "voice_participant_leave",
  "voice_participant_update",
  "voice_stream_publish",
  "voice_stream_unpublish",
]);

export function dispatchVoiceGatewayEvent(
  type: string,
  payload: unknown,
  handlers: VoiceGatewayDispatchHandlers,
): boolean {
  if (!VOICE_GATEWAY_EVENT_TYPES.has(type)) {
    return false;
  }

  const voiceEvent = decodeVoiceGatewayEvent(type, payload);
  if (!voiceEvent) {
    return true;
  }

  if (voiceEvent.type === "voice_participant_sync") {
    handlers.onVoiceParticipantSync?.(voiceEvent.payload);
    return true;
  }
  if (voiceEvent.type === "voice_participant_join") {
    handlers.onVoiceParticipantJoin?.(voiceEvent.payload);
    return true;
  }
  if (voiceEvent.type === "voice_participant_leave") {
    handlers.onVoiceParticipantLeave?.(voiceEvent.payload);
    return true;
  }
  if (voiceEvent.type === "voice_participant_update") {
    handlers.onVoiceParticipantUpdate?.(voiceEvent.payload);
    return true;
  }
  if (voiceEvent.type === "voice_stream_publish") {
    handlers.onVoiceStreamPublish?.(voiceEvent.payload);
    return true;
  }

  handlers.onVoiceStreamUnpublish?.(voiceEvent.payload);
  return true;
}
