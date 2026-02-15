import {
  dispatchReadyGatewayEvent,
} from "../src/lib/gateway-ready-dispatch";

const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

describe("dispatchReadyGatewayEvent", () => {
  it("dispatches valid ready payloads", () => {
    const onReady = vi.fn();

    const handled = dispatchReadyGatewayEvent(
      "ready",
      {
        user_id: DEFAULT_USER_ID,
      },
      { onReady },
    );

    expect(handled).toBe(true);
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady).toHaveBeenCalledWith({ userId: DEFAULT_USER_ID });
  });

  it("fails closed for invalid ready payloads", () => {
    const onReady = vi.fn();

    const handled = dispatchReadyGatewayEvent(
      "ready",
      {
        user_id: "invalid-user-id",
      },
      { onReady },
    );

    expect(handled).toBe(true);
    expect(onReady).not.toHaveBeenCalled();
  });

  it("returns false for non-ready event types", () => {
    const onReady = vi.fn();

    const handled = dispatchReadyGatewayEvent(
      "presence_sync",
      {
        user_id: DEFAULT_USER_ID,
      },
      { onReady },
    );

    expect(handled).toBe(false);
    expect(onReady).not.toHaveBeenCalled();
  });
});