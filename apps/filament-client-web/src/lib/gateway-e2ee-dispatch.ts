import {
  decodeE2eeGatewayEvent,
  type DeviceListUpdatePayload,
  type KeyPackageLowPayload,
  type MlsCommitPayload,
  type MlsMessagePayload,
  type MlsWelcomePayload,
} from "./gateway-e2ee-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface E2eeGatewayDispatchHandlers {
  onDeviceListUpdate?: (payload: DeviceListUpdatePayload) => void;
  onKeyPackageLow?: (payload: KeyPackageLowPayload) => void;
  onMlsMessage?: (payload: MlsMessagePayload) => void;
  onMlsCommit?: (payload: MlsCommitPayload) => void;
  onMlsWelcome?: (payload: MlsWelcomePayload) => void;
}

export const E2EE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "device_list_update",
  "keypackage_low",
  "mls_commit",
  "mls_message",
  "mls_welcome",
];

const E2EE_GATEWAY_EVENT_TYPE_SET = new Set<string>(
  E2EE_GATEWAY_DISPATCH_EVENT_TYPES,
);

type E2eeGatewayEvent = NonNullable<ReturnType<typeof decodeE2eeGatewayEvent>>;

const E2EE_DISPATCH_TABLE: GatewayDispatchTable<
  E2eeGatewayEvent,
  E2eeGatewayDispatchHandlers
> = {
  device_list_update: (eventPayload, eventHandlers) => {
    eventHandlers.onDeviceListUpdate?.(eventPayload);
  },
  keypackage_low: (eventPayload, eventHandlers) => {
    eventHandlers.onKeyPackageLow?.(eventPayload);
  },
  mls_message: (eventPayload, eventHandlers) => {
    eventHandlers.onMlsMessage?.(eventPayload);
  },
  mls_commit: (eventPayload, eventHandlers) => {
    eventHandlers.onMlsCommit?.(eventPayload);
  },
  mls_welcome: (eventPayload, eventHandlers) => {
    eventHandlers.onMlsWelcome?.(eventPayload);
  },
};

export function dispatchE2eeGatewayEvent(
  type: string,
  payload: unknown,
  handlers: E2eeGatewayDispatchHandlers,
): boolean {
  if (!E2EE_GATEWAY_EVENT_TYPE_SET.has(type)) {
    return false;
  }
  const event = decodeE2eeGatewayEvent(type, payload);
  if (!event) {
    return true;
  }
  dispatchDecodedGatewayEvent(event, handlers, E2EE_DISPATCH_TABLE);
  return true;
}
