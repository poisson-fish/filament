import {
  dispatchPresenceGatewayEvent,
} from "../src/lib/gateway-presence-dispatch";

const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("dispatchPresenceGatewayEvent", () => {
  it("dispatches decoded presence events to matching handlers", () => {
    const onPresenceSync = vi.fn();

    const handled = dispatchPresenceGatewayEvent(
      "presence_sync",
      {
        guild_id: DEFAULT_GUILD_ID,
        user_ids: [DEFAULT_USER_ID],
      },
      { onPresenceSync },
    );

    expect(handled).toBe(true);
    expect(onPresenceSync).toHaveBeenCalledTimes(1);
    expect(onPresenceSync).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userIds: [DEFAULT_USER_ID],
    });
  });

  it("fails closed for known presence types with invalid payloads", () => {
    const onPresenceUpdate = vi.fn();

    const handled = dispatchPresenceGatewayEvent(
      "presence_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        user_id: DEFAULT_USER_ID,
        status: "away",
      },
      { onPresenceUpdate },
    );

    expect(handled).toBe(true);
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });

  it("returns false for non-presence event types", () => {
    const onPresenceUpdate = vi.fn();

    const handled = dispatchPresenceGatewayEvent(
      "message_create",
      {},
      { onPresenceUpdate },
    );

    expect(handled).toBe(false);
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });
});
