import type {
  VoiceParticipantJoinPayload,
  VoiceParticipantLeavePayload,
  VoiceParticipantSyncPayload,
  VoiceParticipantUpdatePayload,
  VoiceStreamPublishPayload,
  VoiceStreamUnpublishPayload,
} from "./gateway-contracts";
import {
  decodeVoiceGatewayEvent,
  isVoiceGatewayEventType,
} from "./gateway-voice-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface VoiceGatewayDispatchHandlers {
  onVoiceParticipantSync?: (payload: VoiceParticipantSyncPayload) => void;
  onVoiceParticipantJoin?: (payload: VoiceParticipantJoinPayload) => void;
  onVoiceParticipantLeave?: (payload: VoiceParticipantLeavePayload) => void;
  onVoiceParticipantUpdate?: (payload: VoiceParticipantUpdatePayload) => void;
  onVoiceStreamPublish?: (payload: VoiceStreamPublishPayload) => void;
  onVoiceStreamUnpublish?: (payload: VoiceStreamUnpublishPayload) => void;
}

export const VOICE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "voice_participant_sync",
  "voice_participant_join",
  "voice_participant_leave",
  "voice_participant_update",
  "voice_stream_publish",
  "voice_stream_unpublish",
];

type VoiceGatewayEvent = NonNullable<ReturnType<typeof decodeVoiceGatewayEvent>>;

const VOICE_DISPATCH_TABLE: GatewayDispatchTable<
  VoiceGatewayEvent,
  VoiceGatewayDispatchHandlers
> = {
  voice_participant_sync: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceParticipantSync?.(eventPayload);
  },
  voice_participant_join: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceParticipantJoin?.(eventPayload);
  },
  voice_participant_leave: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceParticipantLeave?.(eventPayload);
  },
  voice_participant_update: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceParticipantUpdate?.(eventPayload);
  },
  voice_stream_publish: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceStreamPublish?.(eventPayload);
  },
  voice_stream_unpublish: (eventPayload, eventHandlers) => {
    eventHandlers.onVoiceStreamUnpublish?.(eventPayload);
  },
};

export function dispatchVoiceGatewayEvent(
  type: string,
  payload: unknown,
  handlers: VoiceGatewayDispatchHandlers,
): boolean {
  if (!isVoiceGatewayEventType(type)) {
    return false;
  }

  const voiceEvent = decodeVoiceGatewayEvent(type, payload);
  if (!voiceEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(voiceEvent, handlers, VOICE_DISPATCH_TABLE);
  return true;
}
