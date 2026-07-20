import {
  type DeviceId,
  type UserId,
  deviceIdFromInput,
  userIdFromInput,
} from "../domain/chat";

const MAX_DEVICE_COUNT = 100;
const MAX_KEYPACKAGE_COUNT = 100;

export interface DeviceListUpdatePayload {
  userId: UserId;
  deviceCount: number;
  createdAtUnix: number;
}

export interface KeyPackageLowPayload {
  deviceId: DeviceId;
  remainingCount: number;
  waterMark: number;
  createdAtUnix: number;
}

type E2eeGatewayEvent =
  | { type: "device_list_update"; payload: DeviceListUpdatePayload }
  | { type: "keypackage_low"; payload: KeyPackageLowPayload };

type E2eeGatewayEventType = E2eeGatewayEvent["type"];
type E2eeEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length
    && actual.every((key, index) => key === required[index]);
}

function isBoundedCount(value: unknown, maximum: number, allowZero: boolean): value is number {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= (allowZero ? 0 : 1)
    && value <= maximum;
}

function isUnixTimestamp(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 1;
}

function parseDeviceListUpdate(payload: unknown): DeviceListUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, ["user_id", "device_count", "created_at_unix"])
    || typeof value.user_id !== "string"
    || !isBoundedCount(value.device_count, MAX_DEVICE_COUNT, true)
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }

  try {
    return {
      userId: userIdFromInput(value.user_id),
      deviceCount: value.device_count,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseKeyPackageLow(payload: unknown): KeyPackageLowPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, ["device_id", "remaining_count", "water_mark", "created_at_unix"])
    || typeof value.device_id !== "string"
    || !isBoundedCount(value.remaining_count, MAX_KEYPACKAGE_COUNT, true)
    || !isBoundedCount(value.water_mark, MAX_KEYPACKAGE_COUNT, false)
    || value.remaining_count >= value.water_mark
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }

  try {
    return {
      deviceId: deviceIdFromInput(value.device_id),
      remainingCount: value.remaining_count,
      waterMark: value.water_mark,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

const E2EE_EVENT_DECODERS: {
  [K in E2eeGatewayEventType]: E2eeEventDecoder<
    Extract<E2eeGatewayEvent, { type: K }>["payload"]
  >;
} = {
  device_list_update: parseDeviceListUpdate,
  keypackage_low: parseKeyPackageLow,
};

export function decodeE2eeGatewayEvent(
  type: string,
  payload: unknown,
): E2eeGatewayEvent | null {
  if (type === "device_list_update") {
    const parsed = E2EE_EVENT_DECODERS.device_list_update(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "keypackage_low") {
    const parsed = E2EE_EVENT_DECODERS.keypackage_low(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  return null;
}
