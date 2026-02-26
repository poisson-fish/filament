const gatewayCompatibilityCounters = new Map<string, number>();

function keyFor(path: string, mode: string): string {
  return `${path}:${mode}`;
}

export const GATEWAY_COMPATIBILITY_PATH_CHANNEL_OVERRIDE_MIGRATION =
  "channel_override_migration";

export const GATEWAY_COMPATIBILITY_MODE_LEGACY_DECODE = "legacy_decode";
export const GATEWAY_COMPATIBILITY_MODE_EXPLICIT_DECODE = "explicit_decode";

export function recordGatewayCompatibilityCounter(path: string, mode: string): void {
  const key = keyFor(path, mode);
  const nextValue = (gatewayCompatibilityCounters.get(key) ?? 0) + 1;
  gatewayCompatibilityCounters.set(key, nextValue);
}

export function gatewayCompatibilityCounterValue(path: string, mode: string): number {
  return gatewayCompatibilityCounters.get(keyFor(path, mode)) ?? 0;
}

export function resetGatewayCompatibilityCounters(): void {
  gatewayCompatibilityCounters.clear();
}
