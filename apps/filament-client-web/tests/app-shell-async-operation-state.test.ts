import { describe, expect, it } from "vitest";
import {
  createIdleAsyncOperationState,
  reduceAsyncOperationState,
} from "../src/features/app-shell/state/async-operation-state";

describe("app shell async operation state", () => {
  it("transitions across start, succeed, fail, and reset", () => {
    const idle = createIdleAsyncOperationState();
    expect(idle).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });

    const running = reduceAsyncOperationState(idle, {
      type: "start",
    });
    expect(running).toEqual({
      phase: "running",
      statusMessage: "",
      errorMessage: "",
    });

    const succeeded = reduceAsyncOperationState(running, {
      type: "succeed",
      statusMessage: "Done.",
    });
    expect(succeeded).toEqual({
      phase: "succeeded",
      statusMessage: "Done.",
      errorMessage: "",
    });

    const failed = reduceAsyncOperationState(succeeded, {
      type: "fail",
      errorMessage: "Failed.",
    });
    expect(failed).toEqual({
      phase: "failed",
      statusMessage: "",
      errorMessage: "Failed.",
    });

    const reset = reduceAsyncOperationState(failed, {
      type: "reset",
    });
    expect(reset).toEqual(idle);
  });
});
