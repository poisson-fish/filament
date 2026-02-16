import type { SystemApi } from "../src/lib/api-system";
import { createSystemClient } from "../src/lib/api-system-client";

describe("api-system-client", () => {
  function createSystemApiStub(overrides?: Partial<SystemApi>): SystemApi {
    const api: SystemApi = {
      fetchHealth: vi.fn(async () => ({ status: "ok" as const })),
      echoMessage: vi.fn(async (input: { message: string }) => input.message),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates fetchHealth through system API", async () => {
    const expected = { status: "ok" as const };
    const fetchHealth = vi.fn(async () => expected);
    const systemClient = createSystemClient({
      systemApi: createSystemApiStub({ fetchHealth }),
    });

    await expect(systemClient.fetchHealth()).resolves.toBe(expected);
    expect(fetchHealth).toHaveBeenCalledTimes(1);
  });

  it("delegates echoMessage through system API", async () => {
    const echoMessage = vi.fn(async () => "echo:hello");
    const systemClient = createSystemClient({
      systemApi: createSystemApiStub({ echoMessage }),
    });

    await expect(systemClient.echoMessage({ message: "hello" })).resolves.toBe("echo:hello");
    expect(echoMessage).toHaveBeenCalledWith({ message: "hello" });
  });
});
