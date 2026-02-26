import {
  duplicateGatewayDomainEventTypes,
  dispatchGatewayDomainEvent,
  GATEWAY_DOMAIN_EVENT_TYPES,
} from "../src/lib/gateway-domain-dispatch";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const DEFAULT_AUTHOR_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

describe("dispatchGatewayDomainEvent", () => {
  it("dispatches handled domain events via the shared dispatch pipeline", () => {
    const onMessageCreate = vi.fn();

    const handled = dispatchGatewayDomainEvent(
      "message_create",
      {
        message_id: DEFAULT_MESSAGE_ID,
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        author_id: DEFAULT_AUTHOR_ID,
        content: "hello",
        markdown_tokens: [
          {
            type: "text",
            text: "hello",
          },
        ],
        attachments: [],
        created_at_unix: 1710000001,
      },
      { onMessageCreate },
    );

    expect(handled).toBe(true);
    expect(onMessageCreate).toHaveBeenCalledTimes(1);
  });

  it("fails closed for known invalid payloads", () => {
    const onMessageUpdate = vi.fn();

    const handled = dispatchGatewayDomainEvent(
      "message_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        message_id: DEFAULT_MESSAGE_ID,
        updated_fields: {},
        updated_at_unix: 1710000010,
      },
      { onMessageUpdate },
    );

    expect(handled).toBe(true);
    expect(onMessageUpdate).not.toHaveBeenCalled();
  });

  it("returns false for unknown event types", () => {
    const onProfileUpdate = vi.fn();

    const handled = dispatchGatewayDomainEvent(
      "unknown_event",
      {},
      { onProfileUpdate },
    );

    expect(handled).toBe(false);
    expect(onProfileUpdate).not.toHaveBeenCalled();
  });

  it("has no duplicate event types in the centralized domain registry", () => {
    expect(duplicateGatewayDomainEventTypes()).toEqual([]);
  });

  it("routes every registered domain event type through the domain dispatcher", () => {
    for (const eventType of GATEWAY_DOMAIN_EVENT_TYPES) {
      const handled = dispatchGatewayDomainEvent(eventType, {}, {});
      expect(handled).toBe(true);
    }
  });
});
