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

export function isVoiceGatewayEventType(value: string): value is VoiceGatewayEventType {
  return isVoiceParticipantGatewayEventType(value) || isVoiceStreamGatewayEventType(value);
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

  if (isVoiceParticipantGatewayEventType(type)) {
    return decodeVoiceParticipantGatewayEvent(type, payload);
  }

  return null;
}