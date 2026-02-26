import type {
  ProfileAvatarUpdatePayload,
  ProfileUpdatePayload,
} from "./gateway-contracts";
import {
  decodeProfileGatewayEvent,
} from "./gateway-profile-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface ProfileGatewayDispatchHandlers {
  onProfileUpdate?: (payload: ProfileUpdatePayload) => void;
  onProfileAvatarUpdate?: (payload: ProfileAvatarUpdatePayload) => void;
}

export const PROFILE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "profile_update",
  "profile_avatar_update",
];

const PROFILE_GATEWAY_EVENT_TYPE_SET = new Set<string>(
  PROFILE_GATEWAY_DISPATCH_EVENT_TYPES,
);

type ProfileGatewayEvent = NonNullable<
  ReturnType<typeof decodeProfileGatewayEvent>
>;

const PROFILE_DISPATCH_TABLE: GatewayDispatchTable<
  ProfileGatewayEvent,
  ProfileGatewayDispatchHandlers
> = {
  profile_update: (eventPayload, eventHandlers) => {
    eventHandlers.onProfileUpdate?.(eventPayload);
  },
  profile_avatar_update: (eventPayload, eventHandlers) => {
    eventHandlers.onProfileAvatarUpdate?.(eventPayload);
  },
};

export function dispatchProfileGatewayEvent(
  type: string,
  payload: unknown,
  handlers: ProfileGatewayDispatchHandlers,
): boolean {
  if (!PROFILE_GATEWAY_EVENT_TYPE_SET.has(type)) {
    return false;
  }

  const profileEvent = decodeProfileGatewayEvent(type, payload);
  if (!profileEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(profileEvent, handlers, PROFILE_DISPATCH_TABLE);
  return true;
}
