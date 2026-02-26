import { GATEWAY_DOMAIN_EVENT_TYPES } from "./gateway-domain-dispatch";
import { READY_GATEWAY_DISPATCH_EVENT_TYPES } from "./gateway-ready-dispatch";

const CLIENT_GATEWAY_EVENT_TYPES = new Set<string>([
  ...READY_GATEWAY_DISPATCH_EVENT_TYPES,
  ...GATEWAY_DOMAIN_EVENT_TYPES,
]);

export const CLIENT_SUPPORTED_GATEWAY_EVENT_TYPES: readonly string[] = Object.freeze(
  Array.from(CLIENT_GATEWAY_EVENT_TYPES).sort(),
);
