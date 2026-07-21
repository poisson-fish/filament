import { dispatchE2eeGatewayEvent } from "../src/lib/gateway-e2ee-dispatch";
import { decodeE2eeGatewayEvent } from "../src/lib/gateway-e2ee-events";

const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEVICE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const GROUP_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const CONVERSATION_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

describe("E2EE gateway events", () => {
  it("strictly decodes and dispatches device-list updates", () => {
    const payload = {
      user_id: USER_ID,
      device_count: 2,
      created_at_unix: 1_710_000_000,
    };
    expect(decodeE2eeGatewayEvent("device_list_update", payload)).toEqual({
      type: "device_list_update",
      payload: {
        userId: USER_ID,
        deviceCount: 2,
        createdAtUnix: 1_710_000_000,
      },
    });

    const onDeviceListUpdate = vi.fn();
    expect(dispatchE2eeGatewayEvent("device_list_update", payload, {
      onDeviceListUpdate,
    })).toBe(true);
    expect(onDeviceListUpdate).toHaveBeenCalledOnce();
  });

  it("decodes low-pool alerts and rejects impossible or extra fields", () => {
    expect(decodeE2eeGatewayEvent("keypackage_low", {
      device_id: DEVICE_ID,
      remaining_count: 4,
      water_mark: 10,
      created_at_unix: 1_710_000_001,
    })).toEqual({
      type: "keypackage_low",
      payload: {
        deviceId: DEVICE_ID,
        remainingCount: 4,
        waterMark: 10,
        createdAtUnix: 1_710_000_001,
      },
    });
    expect(decodeE2eeGatewayEvent("keypackage_low", {
      device_id: DEVICE_ID,
      remaining_count: 10,
      water_mark: 10,
      created_at_unix: 1_710_000_001,
    })).toBeNull();
    expect(decodeE2eeGatewayEvent("device_list_update", {
      user_id: USER_ID,
      device_count: 2,
      created_at_unix: 1_710_000_000,
      extra: true,
    })).toBeNull();
  });

  it("fails closed for malformed known events and ignores unknown types", () => {
    const onKeyPackageLow = vi.fn();
    expect(dispatchE2eeGatewayEvent("keypackage_low", {}, { onKeyPackageLow })).toBe(true);
    expect(onKeyPackageLow).not.toHaveBeenCalled();
    expect(dispatchE2eeGatewayEvent("mls_message", {}, { onKeyPackageLow })).toBe(true);
    expect(dispatchE2eeGatewayEvent("unknown_e2ee_event", {}, { onKeyPackageLow })).toBe(false);
  });

  it("strictly decodes active MLS routing notifications without ciphertext", () => {
    const messagePayload = {
      group_id: GROUP_ID,
      conversation_id: CONVERSATION_ID,
      message_id: MESSAGE_ID,
      epoch: 1,
      suite_id: 3,
      sender_device_id: DEVICE_ID,
      created_at_unix: 1_710_000_002,
    };
    const onMlsMessage = vi.fn();
    expect(dispatchE2eeGatewayEvent("mls_message", messagePayload, { onMlsMessage })).toBe(true);
    expect(onMlsMessage).toHaveBeenCalledWith({
      groupId: GROUP_ID,
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
      epoch: 1,
      suiteId: 3,
      senderDeviceId: DEVICE_ID,
      createdAtUnix: 1_710_000_002,
    });

    expect(decodeE2eeGatewayEvent("mls_commit", {
      group_id: GROUP_ID,
      conversation_id: CONVERSATION_ID,
      epoch: 2,
      prior_epoch: 0,
      committer_device_id: DEVICE_ID,
      created_at_unix: 1_710_000_003,
    })).toBeNull();
    expect(decodeE2eeGatewayEvent("mls_welcome", {
      group_id: GROUP_ID,
      conversation_id: CONVERSATION_ID,
      epoch: 1,
      suite_id: 999,
      created_at_unix: 1_710_000_004,
    })).toBeNull();
  });
});
