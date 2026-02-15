const MAX_GATEWAY_EVENT_BYTES = 64 * 1024;
const EVENT_TYPE_PATTERN = /^[a-z0-9_.]{1,64}$/;
const hasOwn = Object.prototype.hasOwnProperty;

export type GatewayEventEnvelope = {
  v: number;
  t: string;
  d: unknown;
};

export function parseGatewayEventEnvelope(raw: string): GatewayEventEnvelope | null {
  if (new TextEncoder().encode(raw).length > MAX_GATEWAY_EVENT_BYTES) {
    return null;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }

  if (!parsed || typeof parsed !== "object") {
    return null;
  }

  const value = parsed as Record<string, unknown>;
  if (
    !hasOwn.call(value, "v") ||
    !hasOwn.call(value, "t") ||
    !hasOwn.call(value, "d") ||
    value.v !== 1 ||
    typeof value.t !== "string" ||
    !EVENT_TYPE_PATTERN.test(value.t)
  ) {
    return null;
  }

  return {
    v: 1,
    t: value.t,
    d: value.d,
  };
}
