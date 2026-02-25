import {
  dispatchSubscribedGatewayEvent,
  dispatchReadyGatewayEvent,
} from "../src/lib/gateway-ready-dispatch";

const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

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

describe("dispatchSubscribedGatewayEvent", () => {
  it("dispatches valid subscribed payloads", () => {
    const onSubscribed = vi.fn();

    const handled = dispatchSubscribedGatewayEvent(
      "subscribed",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
      },
      { onSubscribed },
    );

    expect(handled).toBe(true);
    expect(onSubscribed).toHaveBeenCalledTimes(1);
    expect(onSubscribed).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
    });
  });

  it("fails closed for invalid subscribed payloads", () => {
    const onSubscribed = vi.fn();

    const handled = dispatchSubscribedGatewayEvent(
      "subscribed",
      {
        guild_id: "invalid-guild-id",
        channel_id: DEFAULT_CHANNEL_ID,
      },
      { onSubscribed },
    );

    expect(handled).toBe(true);
    expect(onSubscribed).not.toHaveBeenCalled();
  });

  it("returns false for non-subscribed event types", () => {
    const onSubscribed = vi.fn();

    const handled = dispatchSubscribedGatewayEvent(
      "ready",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
      },
      { onSubscribed },
    );

    expect(handled).toBe(false);
    expect(onSubscribed).not.toHaveBeenCalled();
  });
});
