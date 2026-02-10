import { accessTokenFromInput } from "../src/domain/auth";
import { resolveGatewayUrl } from "../src/lib/gateway";

describe("gateway URL resolution", () => {
  const token = accessTokenFromInput("A".repeat(64));

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("uses explicit gateway env URL", () => {
    vi.stubEnv("VITE_FILAMENT_GATEWAY_WS_URL", "wss://chat.example.com");
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "https://api.example.com");
    expect(resolveGatewayUrl(token)).toBe(`wss://chat.example.com/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });

  it("derives ws URL from API base URL", () => {
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "https://api.filament.example/api");
    expect(resolveGatewayUrl(token)).toBe(`wss://api.filament.example/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });

  it("falls back to relative gateway path", () => {
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "/api");
    expect(resolveGatewayUrl(token)).toBe(`/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });
});
