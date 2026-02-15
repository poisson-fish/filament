import {
  dispatchFriendGatewayEvent,
  type FriendGatewayDispatchHandlers,
} from "./gateway-friend-dispatch";
import {
  dispatchMessageGatewayEvent,
  type MessageGatewayDispatchHandlers,
} from "./gateway-message-dispatch";
import {
  dispatchPresenceGatewayEvent,
  type PresenceGatewayDispatchHandlers,
} from "./gateway-presence-dispatch";
import {
  dispatchProfileGatewayEvent,
  type ProfileGatewayDispatchHandlers,
} from "./gateway-profile-dispatch";
import {
  dispatchVoiceGatewayEvent,
  type VoiceGatewayDispatchHandlers,
} from "./gateway-voice-dispatch";
import {
  dispatchWorkspaceGatewayEvent,
  type WorkspaceGatewayDispatchHandlers,
} from "./gateway-workspace-dispatch";

export type GatewayDomainDispatchHandlers =
  & MessageGatewayDispatchHandlers
  & WorkspaceGatewayDispatchHandlers
  & ProfileGatewayDispatchHandlers
  & FriendGatewayDispatchHandlers
  & VoiceGatewayDispatchHandlers
  & PresenceGatewayDispatchHandlers;

type DomainDispatcher = (
  type: string,
  payload: unknown,
  handlers: GatewayDomainDispatchHandlers,
) => boolean;

const GATEWAY_DOMAIN_DISPATCHERS: readonly DomainDispatcher[] = [
  dispatchMessageGatewayEvent,
  dispatchWorkspaceGatewayEvent,
  dispatchProfileGatewayEvent,
  dispatchFriendGatewayEvent,
  dispatchVoiceGatewayEvent,
  dispatchPresenceGatewayEvent,
];

export function dispatchGatewayDomainEvent(
  type: string,
  payload: unknown,
  handlers: GatewayDomainDispatchHandlers,
): boolean {
  for (const dispatchDomainEvent of GATEWAY_DOMAIN_DISPATCHERS) {
    if (dispatchDomainEvent(type, payload, handlers)) {
      return true;
    }
  }
  return false;
}