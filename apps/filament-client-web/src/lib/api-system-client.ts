import type { SystemApi } from "./api-system";

interface SystemClientDependencies {
  systemApi: SystemApi;
}

export interface SystemClient {
  fetchHealth(): Promise<{ status: "ok" }>;
  echoMessage(input: { message: string }): Promise<string>;
}

export function createSystemClient(input: SystemClientDependencies): SystemClient {
  return {
    fetchHealth() {
      return input.systemApi.fetchHealth();
    },

    echoMessage(payload) {
      return input.systemApi.echoMessage(payload);
    },
  };
}
