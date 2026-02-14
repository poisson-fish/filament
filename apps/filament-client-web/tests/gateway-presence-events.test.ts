import {
  decodePresenceGatewayEvent,
} from "../src/lib/gateway-presence-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("decodePresenceGatewayEvent", () => {
  it("decodes valid presence_sync payload with deduped users", () => {
    const result = decodePresenceGatewayEvent("presence_sync", {
      guild_id: DEFAULT_GUILD_ID,
      user_ids: [DEFAULT_USER_ID, DEFAULT_USER_ID],
    });

    expect(result).toEqual({
      type: "presence_sync",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        userIds: [DEFAULT_USER_ID],
      },
    });
  });

  it("fails closed for invalid presence_update payload", () => {
    const result = decodePresenceGatewayEvent("presence_update", {
      guild_id: DEFAULT_GUILD_ID,
      user_id: DEFAULT_USER_ID,
      status: "idle",
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodePresenceGatewayEvent("presence_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      user_ids: [DEFAULT_USER_ID],
    });

    expect(result).toBeNull();
  });
});
