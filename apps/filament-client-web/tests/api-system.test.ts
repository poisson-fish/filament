import { createSystemApi } from "../src/lib/api-system";

class MockApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "MockApiError";
    this.status = status;
    this.code = code;
  }
}

describe("api-system", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetchHealth validates strict ok shape", async () => {
    const requestJson = vi.fn(async () => ({ status: "ok" }));
    const api = createSystemApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchHealth()).resolves.toEqual({ status: "ok" });
    expect(requestJson).toHaveBeenCalledWith({ method: "GET", path: "/health" });
  });

  it("fetchHealth fails closed on non-ok shape", async () => {
    const api = createSystemApi({
      requestJson: vi.fn(async () => ({ status: "degraded" })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchHealth()).rejects.toMatchObject({
      status: 500,
      code: "invalid_health_shape",
    });
  });

  it("echoMessage validates strict message response shape", async () => {
    const requestJson = vi.fn(async () => ({ message: "pong" }));
    const api = createSystemApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.echoMessage({ message: "ping" })).resolves.toBe("pong");
    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: "/echo",
      body: { message: "ping" },
    });
  });

  it("echoMessage fails closed on invalid response shape", async () => {
    const api = createSystemApi({
      requestJson: vi.fn(async () => ({ message: 42 })),
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.echoMessage({ message: "ping" })).rejects.toMatchObject({
      status: 500,
      code: "invalid_echo_shape",
    });
  });
});
