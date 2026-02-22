import { createApiTransport } from "../src/lib/api-transport";

describe("api transport base URL resolution", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("uses explicit env base URL when configured", () => {
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "https://chat.example.com/api");

    const transport = createApiTransport();

    expect(transport.apiBaseUrl()).toBe("https://chat.example.com/api");
  });

  it("falls back to same-origin API path by default", () => {
    const transport = createApiTransport();

    expect(transport.apiBaseUrl()).toBe("/api");
  });
});
