import {
  dispatchFriendGatewayEvent,
  FRIEND_GATEWAY_DISPATCH_EVENT_TYPES,
  type FriendGatewayDispatchHandlers,
} from "./gateway-friend-dispatch";
import {
  dispatchMessageGatewayEvent,
  MESSAGE_GATEWAY_DISPATCH_EVENT_TYPES,
  type MessageGatewayDispatchHandlers,
} from "./gateway-message-dispatch";
import {
  dispatchPresenceGatewayEvent,
  PRESENCE_GATEWAY_DISPATCH_EVENT_TYPES,
  type PresenceGatewayDispatchHandlers,
} from "./gateway-presence-dispatch";
import {
  dispatchProfileGatewayEvent,
  PROFILE_GATEWAY_DISPATCH_EVENT_TYPES,
  type ProfileGatewayDispatchHandlers,
} from "./gateway-profile-dispatch";
import {
  dispatchVoiceGatewayEvent,
  VOICE_GATEWAY_DISPATCH_EVENT_TYPES,
  type VoiceGatewayDispatchHandlers,
} from "./gateway-voice-dispatch";
import {
  dispatchWorkspaceGatewayEvent,
  WORKSPACE_GATEWAY_DISPATCH_EVENT_TYPES,
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

interface DomainDispatchRegistration {
  dispatch: DomainDispatcher;
  eventTypes: readonly string[];
}

const GATEWAY_DOMAIN_DISPATCH_REGISTRATIONS: readonly DomainDispatchRegistration[] = [
  {
    dispatch: dispatchMessageGatewayEvent,
    eventTypes: MESSAGE_GATEWAY_DISPATCH_EVENT_TYPES,
  },
  {
    dispatch: dispatchWorkspaceGatewayEvent,
    eventTypes: WORKSPACE_GATEWAY_DISPATCH_EVENT_TYPES,
  },
  {
    dispatch: dispatchProfileGatewayEvent,
    eventTypes: PROFILE_GATEWAY_DISPATCH_EVENT_TYPES,
  },
  {
    dispatch: dispatchFriendGatewayEvent,
    eventTypes: FRIEND_GATEWAY_DISPATCH_EVENT_TYPES,
  },
  {
    dispatch: dispatchVoiceGatewayEvent,
    eventTypes: VOICE_GATEWAY_DISPATCH_EVENT_TYPES,
  },
  {
    dispatch: dispatchPresenceGatewayEvent,
    eventTypes: PRESENCE_GATEWAY_DISPATCH_EVENT_TYPES,
  },
];

export function duplicateGatewayDomainEventTypes(
  registrations: readonly DomainDispatchRegistration[] = GATEWAY_DOMAIN_DISPATCH_REGISTRATIONS,
): string[] {
  const counts = new Map<string, number>();
  for (const registration of registrations) {
    for (const eventType of registration.eventTypes) {
      counts.set(eventType, (counts.get(eventType) ?? 0) + 1);
    }
  }

  const duplicates: string[] = [];
  for (const [eventType, count] of counts.entries()) {
    if (count > 1) {
      duplicates.push(eventType);
    }
  }
  duplicates.sort();
  return duplicates;
}

function gatewayDomainDispatchRegistry(): ReadonlyMap<string, DomainDispatcher> {
  const duplicates = duplicateGatewayDomainEventTypes();
  if (duplicates.length > 0) {
    throw new Error(`duplicate gateway domain event types: ${duplicates.join(", ")}`);
  }

  const registry = new Map<string, DomainDispatcher>();
  for (const registration of GATEWAY_DOMAIN_DISPATCH_REGISTRATIONS) {
    for (const eventType of registration.eventTypes) {
      registry.set(eventType, registration.dispatch);
    }
  }
  return registry;
}

const GATEWAY_DOMAIN_DISPATCH_REGISTRY = gatewayDomainDispatchRegistry();

export const GATEWAY_DOMAIN_EVENT_TYPES: readonly string[] = Object.freeze(
  Array.from(GATEWAY_DOMAIN_DISPATCH_REGISTRY.keys()).sort(),
);

export function dispatchGatewayDomainEvent(
  type: string,
  payload: unknown,
  handlers: GatewayDomainDispatchHandlers,
): boolean {
  const domainDispatcher = GATEWAY_DOMAIN_DISPATCH_REGISTRY.get(type);
  return domainDispatcher ? domainDispatcher(type, payload, handlers) : false;
}
