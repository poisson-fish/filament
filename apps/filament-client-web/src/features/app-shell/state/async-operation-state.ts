export type AsyncOperationPhase = "idle" | "running" | "succeeded" | "failed";

export interface AsyncOperationState {
  phase: AsyncOperationPhase;
  statusMessage: string;
  errorMessage: string;
}

export type AsyncOperationEvent =
  | { type: "reset" }
  | { type: "start" }
  | { type: "succeed"; statusMessage?: string }
  | { type: "fail"; errorMessage: string };

export function createIdleAsyncOperationState(): AsyncOperationState {
  return {
    phase: "idle",
    statusMessage: "",
    errorMessage: "",
  };
}

export function reduceAsyncOperationState(
  state: AsyncOperationState,
  event: AsyncOperationEvent,
): AsyncOperationState {
  if (event.type === "reset") {
    return createIdleAsyncOperationState();
  }

  if (event.type === "start") {
    return {
      phase: "running",
      statusMessage: "",
      errorMessage: "",
    };
  }

  if (event.type === "succeed") {
    return {
      phase: "succeeded",
      statusMessage: event.statusMessage ?? "",
      errorMessage: "",
    };
  }

  return {
    phase: "failed",
    statusMessage: "",
    errorMessage: event.errorMessage,
  };
}