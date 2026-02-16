interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
}

interface SystemApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  createApiError: (status: number, code: string, message: string) => Error;
}

export interface SystemApi {
  fetchHealth(): Promise<{ status: "ok" }>;
  echoMessage(input: { message: string }): Promise<string>;
}

export function createSystemApi(input: SystemApiDependencies): SystemApi {
  return {
    async fetchHealth() {
      const dto = await input.requestJson({
        method: "GET",
        path: "/health",
      });

      if (!dto || typeof dto !== "object" || (dto as { status?: unknown }).status !== "ok") {
        throw input.createApiError(500, "invalid_health_shape", "Unexpected health response.");
      }
      return { status: "ok" };
    },

    async echoMessage(payload) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/echo",
        body: { message: payload.message },
      });

      if (
        !dto ||
        typeof dto !== "object" ||
        typeof (dto as { message?: unknown }).message !== "string"
      ) {
        throw input.createApiError(500, "invalid_echo_shape", "Unexpected echo response.");
      }
      return (dto as { message: string }).message;
    },
  };
}
