import {
  decodeVoiceParticipantGatewayEvent,
  isVoiceParticipantGatewayEventType,
  type VoiceParticipantGatewayEvent,
} from "./gateway-voice-participant-events";
import {
  decodeVoiceStreamGatewayEvent,
  isVoiceStreamGatewayEventType,
  type VoiceStreamGatewayEvent,
} from "./gateway-voice-stream-events";

type VoiceGatewayEvent = VoiceParticipantGatewayEvent | VoiceStreamGatewayEvent;

type VoiceGatewayEventType = VoiceGatewayEvent["type"];
type VoiceGatewayEventDecoder = (
  type: string,
  payload: unknown,
) => VoiceGatewayEvent | null;

const VOICE_EVENT_TYPE_GUARDS: ReadonlyArray<(value: string) => boolean> = [
  isVoiceParticipantGatewayEventType,
  isVoiceStreamGatewayEventType,
];

const VOICE_EVENT_DECODER_REGISTRY: ReadonlyArray<VoiceGatewayEventDecoder> = [
  decodeVoiceParticipantGatewayEvent,
  decodeVoiceStreamGatewayEvent,
];

export function isVoiceGatewayEventType(value: string): value is VoiceGatewayEventType {
  return VOICE_EVENT_TYPE_GUARDS.some((guard) => guard(value));
}

export function decodeVoiceGatewayEvent(
  type: string,
  payload: unknown,
): VoiceGatewayEvent | null {
  if (!isVoiceGatewayEventType(type)) {
    return null;
  }

  for (const decoder of VOICE_EVENT_DECODER_REGISTRY) {
    const decodedEvent = decoder(type, payload);
    if (decodedEvent) {
      return decodedEvent;
    }
  }

  return null;
}