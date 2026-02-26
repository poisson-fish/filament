import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { CLIENT_SUPPORTED_GATEWAY_EVENT_TYPES } from "../src/lib/gateway-event-manifest";

interface GatewayManifestEntry {
  event_type: string;
  schema_version: number;
  scope: "connection" | "channel" | "guild" | "user";
  lifecycle: "active" | "deprecated";
  migration?: string;
}

interface GatewayManifest {
  events: GatewayManifestEntry[];
}

function loadProtocolGatewayManifest(): GatewayManifest {
  const currentDir = dirname(fileURLToPath(import.meta.url));
  const manifestPath = resolve(
    currentDir,
    "../../../crates/filament-protocol/src/events/gateway_events_manifest.json",
  );
  return JSON.parse(readFileSync(manifestPath, "utf8")) as GatewayManifest;
}

describe("gateway protocol manifest parity", () => {
  it("matches client supported event registry", () => {
    const manifest = loadProtocolGatewayManifest();
    const manifestEvents = manifest.events.map((entry) => entry.event_type).sort();

    expect(manifestEvents).toEqual(CLIENT_SUPPORTED_GATEWAY_EVENT_TYPES);
  });

  it("requires migration notes for deprecated events", () => {
    const manifest = loadProtocolGatewayManifest();
    for (const event of manifest.events) {
      expect(event.schema_version).toBeGreaterThan(0);
      if (event.lifecycle === "deprecated") {
        expect(typeof event.migration).toBe("string");
        expect(event.migration?.trim().length ?? 0).toBeGreaterThan(0);
      }
    }
  });
});
