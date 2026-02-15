import type {
  ProfileAvatarUpdatePayload,
  ProfileUpdatePayload,
} from "./gateway-contracts";
import {
  decodeProfileGatewayEvent,
} from "./gateway-profile-events";

export interface ProfileGatewayDispatchHandlers {
  onProfileUpdate?: (payload: ProfileUpdatePayload) => void;
  onProfileAvatarUpdate?: (payload: ProfileAvatarUpdatePayload) => void;
}

const PROFILE_GATEWAY_EVENT_TYPES = new Set<string>([
  "profile_update",
  "profile_avatar_update",
]);

export function dispatchProfileGatewayEvent(
  type: string,
  payload: unknown,
  handlers: ProfileGatewayDispatchHandlers,
): boolean {
  if (!PROFILE_GATEWAY_EVENT_TYPES.has(type)) {
    return false;
  }

  const profileEvent = decodeProfileGatewayEvent(type, payload);
  if (!profileEvent) {
    return true;
  }

  if (profileEvent.type === "profile_update") {
    handlers.onProfileUpdate?.(profileEvent.payload);
    return true;
  }

  handlers.onProfileAvatarUpdate?.(profileEvent.payload);
  return true;
}
