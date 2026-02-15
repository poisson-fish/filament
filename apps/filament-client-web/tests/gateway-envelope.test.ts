import { parseGatewayEventEnvelope } from "../src/lib/gateway-envelope";

describe("parseGatewayEventEnvelope", () => {
  it("parses valid versioned envelope", () => {
    const result = parseGatewayEventEnvelope(
      JSON.stringify({ v: 1, t: "presence_sync", d: { guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV" } }),
    );

    expect(result).toEqual({
      v: 1,
      t: "presence_sync",
      d: { guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV" },
    });
  });

  it("rejects unknown envelope version", () => {
    const result = parseGatewayEventEnvelope(JSON.stringify({ v: 2, t: "ready", d: {} }));
    expect(result).toBeNull();
  });

  it("rejects event types outside strict pattern", () => {
    const result = parseGatewayEventEnvelope(JSON.stringify({ v: 1, t: "presence-sync", d: {} }));
    expect(result).toBeNull();
  });

  it("rejects oversized payload before parsing", () => {
    const result = parseGatewayEventEnvelope("x".repeat(70 * 1024));
    expect(result).toBeNull();
  });

  it("rejects non-json input", () => {
    const result = parseGatewayEventEnvelope("not-json");
    expect(result).toBeNull();
  });

  it("rejects envelope missing d field", () => {
    const result = parseGatewayEventEnvelope(JSON.stringify({ v: 1, t: "ready" }));
    expect(result).toBeNull();
  });
});
